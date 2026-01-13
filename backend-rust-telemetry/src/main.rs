use axum::{
    extract::Query,
    extract::{Multipart, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures_core::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    env,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tokio::sync::broadcast;
use tracing::{info, warn};

#[derive(Debug, Serialize)]
struct TelemetryPayload {
    ts_ms: u128,
    cpu_percent: f32,
    mem_total: u64,
    mem_used: u64,
    mem_free: u64,
    process_count: usize,
}

#[derive(Debug, Serialize, Clone)]
struct MediaRecordedEvent {
    ts_ms: u128,
    twin_id: String,
    filename: String,
    mime_type: String,
    size_bytes: u64,
    stored_path: String,
}

#[derive(Clone)]
struct AppState {
    storage_dir: PathBuf,
    media_tx: broadcast::Sender<MediaRecordedEvent>,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis()
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
}

fn ext_from_mime_or_name(mime: &str, filename: &str) -> Option<&'static str> {
    let name = filename.to_ascii_lowercase();
    if name.ends_with(".webm") {
        return Some("webm");
    }
    if name.ends_with(".mp4") {
        return Some("mp4");
    }

    let mime = mime.to_ascii_lowercase();
    if mime.contains("webm") {
        return Some("webm");
    }
    if mime.contains("mp4") {
        return Some("mp4");
    }
    None
}

async fn sse_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let interval_ms: u64 = env::var("TELEMETRY_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2_000);

    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));

    // sysinfo CPU usage is computed across refreshes. We refresh on each tick.
    let refresh_kind = RefreshKind::new()
        .with_cpu(CpuRefreshKind::new().with_cpu_usage())
        .with_memory(MemoryRefreshKind::new().with_ram());
    let mut sys = System::new_with_specifics(refresh_kind);

    let mut media_rx = state.media_tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    sys.refresh_cpu_usage();
                    sys.refresh_memory();
                    sys.refresh_processes();

                    let cpu_percent: f32 = {
                        let cpus = sys.cpus();
                        if cpus.is_empty() {
                            0.0
                        } else {
                            let sum: f32 = cpus.iter().map(|c| c.cpu_usage()).sum();
                            sum / (cpus.len() as f32)
                        }
                    };

                    // Note: sysinfo memory units may differ by platform/version; treat these as raw units.
                    let total = sys.total_memory();
                    let used = sys.used_memory();
                    let free = total.saturating_sub(used);

                    let payload = TelemetryPayload {
                        ts_ms: now_ms(),
                        cpu_percent,
                        mem_total: total,
                        mem_used: used,
                        mem_free: free,
                        process_count: sys.processes().len(),
                    };

                    let json = match serde_json::to_string(&payload) {
                        Ok(v) => v,
                        Err(_) => "{}".to_string(),
                    };

                    yield Ok(Event::default().event("metrics").data(json));
                }
                evt = media_rx.recv() => {
                    match evt {
                        Ok(media_evt) => {
                            let json = match serde_json::to_string(&media_evt) {
                                Ok(v) => v,
                                Err(_) => "{}".to_string(),
                            };
                            yield Ok(Event::default().event("media").data(json));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // channel closed; ignore
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!(skipped = skipped, "media event stream lagged");
                        }
                    }
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)).text("keep-alive"))
}

#[derive(Debug, Serialize)]
struct MediaUploadResponse {
    ok: bool,
    filename: String,
    size_bytes: u64,
    stored_path: String,
}

#[derive(Debug, Deserialize)]
struct MediaListQuery {
    #[serde(default)]
    twin_id: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct MediaListItem {
    filename: String,
    size_bytes: u64,
    stored_path: String,
    ts_ms: Option<u128>,
}

#[derive(Debug, Serialize)]
struct MediaListResponse {
    recordings: Vec<MediaListItem>,
}

async fn media_list_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<MediaListQuery>,
) -> Result<Json<MediaListResponse>, (axum::http::StatusCode, String)> {
    let requested_twin = q.twin_id.trim().to_string();
    let requested_twin = if requested_twin.is_empty() {
        None
    } else {
        Some(sanitize_id(&requested_twin))
    };

    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    let media_dir = state.storage_dir.join("media");

    let mut items: Vec<MediaListItem> = Vec::new();

    if tokio::fs::metadata(&media_dir).await.is_err() {
        return Ok(Json(MediaListResponse { recordings: vec![] }));
    }

    let mut rd = tokio::fs::read_dir(&media_dir)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("read_dir failed: {e}")))?;

    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("read_dir entry failed: {e}")))?
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let filename = match path.file_name().and_then(|v| v.to_str()) {
            Some(v) => v.to_string(),
            None => continue,
        };

        if let Some(twin) = &requested_twin {
            let prefix = format!("rec_{twin}_");
            if !filename.starts_with(&prefix) {
                continue;
            }
        }

        let size_bytes = match tokio::fs::metadata(&path).await {
            Ok(m) => m.len(),
            Err(_) => 0,
        };

        // Parse ts_ms from `rec_<twin>_<ts>.ext`
        let ts_ms = filename
            .rsplit_once('.')
            .and_then(|(base, _ext)| base.rsplit_once('_').map(|(_, ts)| ts))
            .and_then(|ts| ts.parse::<u128>().ok());

        items.push(MediaListItem {
            filename: filename.clone(),
            size_bytes,
            stored_path: path.to_string_lossy().to_string(),
            ts_ms,
        });

        if items.len() >= limit {
            break;
        }
    }

    // Sort newest-first when we can parse timestamps.
    items.sort_by(|a, b| b.ts_ms.cmp(&a.ts_ms));

    Ok(Json(MediaListResponse { recordings: items }))
}

async fn media_upload_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<MediaUploadResponse>, (axum::http::StatusCode, String)> {
    let mut twin_id: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let mut original_filename: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("multipart parse error: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            mime_type = field.content_type().map(|v| v.to_string()).or(mime_type);
            original_filename = field.file_name().map(|v| v.to_string());
            let bytes = field
                .bytes()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("file read error: {e}")))?;
            file_bytes = Some(bytes.to_vec());
        } else if name == "user_id" || name == "twin_id" {
            let v = field
                .text()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("field read error: {e}")))?;
            if !v.trim().is_empty() {
                twin_id = Some(v.trim().to_string());
            }
        } else if name == "mime_type" {
            let v = field
                .text()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("field read error: {e}")))?;
            if !v.trim().is_empty() {
                mime_type = Some(v.trim().to_string());
            }
        } else {
            // ignore other metadata fields for now
        }
    }

    let twin_id = sanitize_id(twin_id.as_deref().unwrap_or("unknown"));
    let bytes = file_bytes.ok_or_else(|| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "missing 'file' field".to_string(),
        )
    })?;

    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let original_filename = original_filename.unwrap_or_else(|| "recording".to_string());
    let ext = ext_from_mime_or_name(&mime_type, &original_filename).ok_or_else(|| {
        (
            axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("unsupported recording type (mime={mime_type}, filename={original_filename})"),
        )
    })?;

    let ts_ms = now_ms();
    let filename = format!("rec_{twin_id}_{ts_ms}.{ext}");

    let media_dir = state.storage_dir.join("media");
    tokio::fs::create_dir_all(&media_dir)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("mkdir failed: {e}")))?;

    let stored_path = media_dir.join(&filename);
    tokio::fs::write(&stored_path, &bytes)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("write failed: {e}")))?;

    let stored_path_str = stored_path.to_string_lossy().to_string();
    info!(
        twin_id = %twin_id,
        mime_type = %mime_type,
        size_bytes = bytes.len(),
        path = %stored_path_str,
        "Stored recording"
    );

    let evt = MediaRecordedEvent {
        ts_ms,
        twin_id: twin_id.clone(),
        filename: filename.clone(),
        mime_type: mime_type.clone(),
        size_bytes: bytes.len() as u64,
        stored_path: stored_path_str.clone(),
    };
    if let Err(e) = state.media_tx.send(evt) {
        warn!(error = %e, "failed to broadcast media event");
    }

    Ok(Json(MediaUploadResponse {
        ok: true,
        filename,
        size_bytes: bytes.len() as u64,
        stored_path: stored_path_str,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust_telemetry=info,axum=info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let port: u16 = env::var("TELEMETRY_PORT")
        .unwrap_or_else(|_| "8183".to_string())
        .parse()
        .expect("TELEMETRY_PORT must be a valid port number");

    let storage_dir = env::var("TELEMETRY_STORAGE_DIR").unwrap_or_else(|_| "./telemetry_storage".to_string());
    let storage_dir = Path::new(storage_dir.trim()).to_path_buf();

    let (media_tx, _media_rx) = broadcast::channel::<MediaRecordedEvent>(128);
    let state = Arc::new(AppState {
        storage_dir,
        media_tx,
    });

    let app = Router::new()
        .route("/v1/telemetry/stream", get(sse_stream))
        .route("/v1/media/upload", post(media_upload_handler))
        .route("/v1/media/list", get(media_list_handler))
        .with_state(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!(addr = %addr, "Starting Telemetry SSE service");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

