use axum::{
    extract::Query,
    extract::{Multipart, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post},
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
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tracing::{info, warn};

mod media;
mod transcription_worker;
mod orchestrator_client;

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
    assets_dir: PathBuf,
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
    #[serde(default)]
    has_transcript: bool,
    #[serde(default)]
    has_summary: bool,
}

#[derive(Debug, Serialize)]
struct MediaListResponse {
    recordings: Vec<MediaListItem>,
}

async fn internal_media_store_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<MediaUploadResponse>, (axum::http::StatusCode, String)> {
    let mut twin_id: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let mut original_filename: Option<String> = None;
    let mut stored: Option<MediaUploadResponse> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("multipart parse error: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            // If the client sends the file field before twin_id, we will fall back to "unknown".
            let twin_id = sanitize_id(twin_id.as_deref().unwrap_or("unknown"));
            let mime_type = field
                .content_type()
                .map(|v| v.to_string())
                .or_else(|| mime_type.clone())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let original_filename = field
                .file_name()
                .map(|v| v.to_string())
                .or_else(|| original_filename.clone())
                .unwrap_or_else(|| "recording.webm".to_string());

            // Bare-metal convention: force webm naming (frontend currently produces webm recordings).
            let ts_ms = now_ms();
            let filename = format!("rec_{twin_id}_{ts_ms}.webm");

            // Ensure storage/recordings exists.
            let recordings_dir = state.storage_dir.join("recordings");
            tokio::fs::create_dir_all(&recordings_dir)
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("mkdir failed: {e}")))?;

            let stored_path = recordings_dir.join(&filename);
            let mut f = tokio::fs::File::create(&stored_path)
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("create failed: {e}")))?;

            let mut size_bytes: u64 = 0;
            let mut field = field;
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("file read error: {e}")))?
            {
                f.write_all(&chunk)
                    .await
                    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("write failed: {e}")))?;
                size_bytes = size_bytes.saturating_add(chunk.len() as u64);
            }

            f.flush()
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("flush failed: {e}")))?;
            drop(f);

            let stored_path_str = stored_path.to_string_lossy().to_string();
            info!(
                twin_id = %twin_id,
                mime_type = %mime_type,
                original_filename = %original_filename,
                size_bytes = size_bytes,
                path = %stored_path_str,
                "Stored recording (internal)"
            );

            let evt = MediaRecordedEvent {
                ts_ms,
                twin_id: twin_id.clone(),
                filename: filename.clone(),
                mime_type: mime_type.clone(),
                size_bytes,
                stored_path: stored_path_str.clone(),
            };

            // Append to an on-disk JSONL log so other services can reference this as multi-modal context.
            let log_path = recordings_dir.join("recordings.jsonl");
            let mut log = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("open log failed: {e}")))?;
            let line = serde_json::to_string(&evt).unwrap_or_else(|_| "{}".to_string());
            log.write_all(line.as_bytes())
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("log write failed: {e}")))?;
            log.write_all(b"\n")
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("log write failed: {e}")))?;

            if let Err(e) = state.media_tx.send(evt) {
                warn!(error = %e, "failed to broadcast media event");
            }

            stored = Some(MediaUploadResponse {
                ok: true,
                filename,
                size_bytes,
                stored_path: stored_path_str,
            });
            break;
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
        } else if name == "original_filename" {
            let v = field
                .text()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("field read error: {e}")))?;
            if !v.trim().is_empty() {
                original_filename = Some(v.trim().to_string());
            }
        } else {
            // ignore other fields
        }
    }

    let stored = stored.ok_or_else(|| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "missing 'file' field".to_string(),
        )
    })?;

    Ok(Json(stored))
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
    // New internal store endpoint writes to recordings/; legacy public upload wrote to media/.
    let recordings_dir = state.storage_dir.join("recordings");
    let legacy_media_dir = state.storage_dir.join("media");

    let mut items: Vec<MediaListItem> = Vec::new();

    for dir in [&recordings_dir, &legacy_media_dir] {
        if tokio::fs::metadata(dir).await.is_err() {
            continue;
        }

        let mut rd = tokio::fs::read_dir(dir)
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

            // Only process video files (webm, mp4, etc.)
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext != "webm" && ext != "mp4" && ext != "mkv" && ext != "avi" {
                continue;
            }

            // Parse ts_ms from `rec_<twin>_<ts>.ext`
            let ts_ms = filename
                .rsplit_once('.')
                .and_then(|(base, _ext)| base.rsplit_once('_').map(|(_, ts)| ts))
                .and_then(|ts| ts.parse::<u128>().ok());

            // Check for transcript and summary files
            let base_name = filename.rsplit_once('.').map(|(base, _)| base).unwrap_or(&filename);
            let transcript_filename = format!("{}.txt", base_name);
            let summary_filename = format!("{}.summary.json", base_name);
            
            let transcript_path = dir.join(&transcript_filename);
            let summary_path = dir.join(&summary_filename);
            
            let has_transcript = tokio::fs::metadata(&transcript_path).await.is_ok();
            let has_summary = tokio::fs::metadata(&summary_path).await.is_ok();

            items.push(MediaListItem {
                filename: filename.clone(),
                size_bytes,
                stored_path: path.to_string_lossy().to_string(),
                ts_ms,
                has_transcript,
                has_summary,
            });

            if items.len() >= limit {
                break;
            }
        }

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
    // Legacy /v1/media/upload implementation (buffers to memory). Kept for compatibility.
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
            // ignore other fields
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
        "Stored recording (legacy)"
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

#[derive(Debug, Serialize)]
struct TranscriptResponse {
    transcript: String,
}

async fn internal_media_transcript_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Result<Json<TranscriptResponse>, (axum::http::StatusCode, String)> {
    // Extract base name from filename (remove extension)
    let base_name = filename.rsplit_once('.').map(|(base, _)| base).unwrap_or(&filename);
    let transcript_filename = format!("{}.txt", base_name);
    
    // Check in recordings directory first, then legacy media directory
    let recordings_dir = state.storage_dir.join("recordings");
    let legacy_media_dir = state.storage_dir.join("media");
    
    let transcript_path = recordings_dir.join(&transcript_filename);
    let transcript_path_legacy = legacy_media_dir.join(&transcript_filename);
    
    let path = if tokio::fs::metadata(&transcript_path).await.is_ok() {
        transcript_path
    } else if tokio::fs::metadata(&transcript_path_legacy).await.is_ok() {
        transcript_path_legacy
    } else {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            format!("Transcript not found for {}", filename),
        ));
    };
    
    let transcript_text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read transcript file: {}", e),
            )
        })?;
    
    Ok(Json(TranscriptResponse {
        transcript: transcript_text,
    }))
}

#[derive(Debug, Serialize)]
struct SummaryResponse {
    insights: SummaryJson,
}

#[derive(Debug, Serialize, Deserialize)]
struct SummaryJson {
    summary: String,
    key_decisions: Vec<String>,
    follow_up_tasks: Vec<String>,
}

async fn internal_media_summary_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Result<Json<SummaryResponse>, (axum::http::StatusCode, String)> {
    // Extract base name from filename (remove extension)
    let base_name = filename.rsplit_once('.').map(|(base, _)| base).unwrap_or(&filename);
    let summary_filename = format!("{}.summary.json", base_name);
    
    // Check in recordings directory first, then legacy media directory
    let recordings_dir = state.storage_dir.join("recordings");
    let legacy_media_dir = state.storage_dir.join("media");
    
    let summary_path = recordings_dir.join(&summary_filename);
    let summary_path_legacy = legacy_media_dir.join(&summary_filename);
    
    let path = if tokio::fs::metadata(&summary_path).await.is_ok() {
        summary_path
    } else if tokio::fs::metadata(&summary_path_legacy).await.is_ok() {
        summary_path_legacy
    } else {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            format!("Summary not found for {}", filename),
        ));
    };
    
    let summary_json_text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read summary file: {}", e),
            )
        })?;
    
    let summary_json: SummaryJson = serde_json::from_str(&summary_json_text)
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse summary JSON: {}", e),
            )
        })?;
    
    Ok(Json(SummaryResponse {
        insights: summary_json,
    }))
}

async fn internal_media_delete_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;
    
    // Check in recordings directory first, then legacy media directory
    let recordings_dir = state.storage_dir.join("recordings");
    let legacy_media_dir = state.storage_dir.join("media");
    
    let file_path = recordings_dir.join(&filename);
    let file_path_legacy = legacy_media_dir.join(&filename);
    
    let path = if tokio::fs::metadata(&file_path).await.is_ok() {
        file_path
    } else if tokio::fs::metadata(&file_path_legacy).await.is_ok() {
        file_path_legacy
    } else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Media file not found: {}", filename),
        ));
    };
    
    // Also try to delete associated transcript and summary files
    let transcript_path = recordings_dir.join(format!("{}.txt", filename));
    let summary_path = recordings_dir.join(format!("{}.summary.json", filename));
    let transcript_path_legacy = legacy_media_dir.join(format!("{}.txt", filename));
    let summary_path_legacy = legacy_media_dir.join(format!("{}.summary.json", filename));
    
    // Delete main file
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete file: {}", e),
            )
        })?;
    
    // Try to delete transcript (ignore errors if it doesn't exist)
    let _ = tokio::fs::remove_file(&transcript_path).await;
    let _ = tokio::fs::remove_file(&transcript_path_legacy).await;
    
    // Try to delete summary (ignore errors if it doesn't exist)
    let _ = tokio::fs::remove_file(&summary_path).await;
    let _ = tokio::fs::remove_file(&summary_path_legacy).await;
    
    Ok(Json(serde_json::json!({ "ok": true, "filename": filename })))
}

async fn internal_media_view_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(filename): axum::extract::Path<String>,
    request: axum::http::Request<axum::body::Body>,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    use axum::body::Body;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::Response;
    
    // Check in recordings directory first, then legacy media directory
    let recordings_dir = state.storage_dir.join("recordings");
    let legacy_media_dir = state.storage_dir.join("media");
    
    let file_path = recordings_dir.join(&filename);
    let file_path_legacy = legacy_media_dir.join(&filename);
    
    let path = if tokio::fs::metadata(&file_path).await.is_ok() {
        file_path
    } else if tokio::fs::metadata(&file_path_legacy).await.is_ok() {
        file_path_legacy
    } else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Media file not found: {}", filename),
        ));
    };
    
    let file_bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read file: {}", e),
            )
        })?;
    
    // Determine content type from extension
    let content_type = match path.extension().and_then(|s| s.to_str()) {
        Some("webm") => "video/webm",
        Some("mp4") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        _ => "application/octet-stream",
    };
    
    let mut response = Response::new(Body::from(file_bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type),
    );
    response.headers_mut().insert(
        header::ACCEPT_RANGES,
        HeaderValue::from_static("bytes"),
    );
    
    // Handle Range requests for video scrubbing
    if let Some(range_header) = request.headers().get(header::RANGE) {
        // For simplicity, we'll return the full file. In production, you'd parse the range
        // and return partial content with 206 Partial Content status.
        // This is a basic implementation.
    }
    
    Ok(response)
}

async fn internal_asset_upload_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let mut asset_type: Option<String> = None;
    let mut stored_path: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("multipart parse error: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            let asset_type_val = asset_type.as_deref().unwrap_or("logo");
            
            // Determine filename based on asset type
            let filename = match asset_type_val {
                "logo" => "custom-logo.svg",
                "favicon" => "custom-favicon.ico",
                "favicon-png" => "custom-favicon-32.png",
                _ => return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Invalid asset type: {}", asset_type_val),
                )),
            };

            let assets_dir = &state.assets_dir;
            tokio::fs::create_dir_all(assets_dir)
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("mkdir failed: {e}")))?;

            let stored_path_buf = assets_dir.join(&filename);
            let mut f = tokio::fs::File::create(&stored_path_buf)
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("create failed: {e}")))?;

            let mut size_bytes: u64 = 0;
            let mut field = field;
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("file read error: {e}")))?
            {
                f.write_all(&chunk)
                    .await
                    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("write failed: {e}")))?;
                size_bytes = size_bytes.saturating_add(chunk.len() as u64);
            }

            f.flush()
                .await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("flush failed: {e}")))?;
            drop(f);

            stored_path = Some(stored_path_buf.to_string_lossy().to_string());
            info!(
                asset_type = %asset_type_val,
                path = %stored_path.as_ref().unwrap(),
                size_bytes = size_bytes,
                "Stored custom asset"
            );
            break;
        } else if name == "asset_type" {
            let v = field
                .text()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, format!("field read error: {e}")))?;
            if !v.trim().is_empty() {
                asset_type = Some(v.trim().to_string());
            }
        }
    }

    let stored_path = stored_path.ok_or_else(|| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "missing 'file' field".to_string(),
        )
    })?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "asset_type": asset_type.unwrap_or("unknown".to_string()),
        "stored_path": stored_path
    })))
}

async fn internal_asset_view_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    use axum::body::Body;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::Response;
    
    let asset_path = state.assets_dir.join(&filename);
    
    // Check if custom asset exists, otherwise return 404
    if tokio::fs::metadata(&asset_path).await.is_err() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Asset not found: {}", filename),
        ));
    }
    
    let file_bytes = tokio::fs::read(&asset_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read file: {}", e),
            )
        })?;
    
    // Determine content type from extension
    let content_type = match asset_path.extension().and_then(|s| s.to_str()) {
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };
    
    let mut response = Response::new(Body::from(file_bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Invalid content type".to_string())
        })?,
    );
    
    Ok(response)
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

    // Bare-metal default: ./storage (recordings will live under ./storage/recordings).
    let storage_dir = env::var("TELEMETRY_STORAGE_DIR").unwrap_or_else(|_| "./storage".to_string());
    let storage_dir = Path::new(storage_dir.trim()).to_path_buf();
    let assets_dir = storage_dir.join("assets");

    // Get gRPC service addresses for transcription worker
    let orchestrator_grpc_addr = env::var("ORCHESTRATOR_GRPC_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50057".to_string());
    let memory_grpc_addr = env::var("MEMORY_GRPC_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50052".to_string());

    let (media_tx, _media_rx) = broadcast::channel::<MediaRecordedEvent>(128);
    let state = Arc::new(AppState { 
        storage_dir: storage_dir.clone(), 
        media_tx,
        assets_dir,
    });

    // Start transcription worker in background
    let storage_dir_worker = storage_dir.clone();
    let orchestrator_addr_worker = orchestrator_grpc_addr.clone();
    let memory_addr_worker = memory_grpc_addr.clone();
    tokio::spawn(async move {
        if let Err(e) = transcription_worker::start_transcription_watcher(
            storage_dir_worker,
            orchestrator_addr_worker,
            memory_addr_worker,
        )
        .await
        {
            warn!(error = %e, "Transcription worker exited with error");
        }
    });

    info!(
        orchestrator_addr = %orchestrator_grpc_addr,
        memory_addr = %memory_grpc_addr,
        "Started transcription worker"
    );

    let app = Router::new()
        .route("/v1/telemetry/stream", get(sse_stream))
        .route("/v1/media/upload", post(media_upload_handler))
        .route("/v1/media/list", get(media_list_handler))
        .route("/internal/media/store", post(internal_media_store_handler))
        .route("/internal/media/view/:filename", get(internal_media_view_handler))
        .route("/internal/media/transcript/:filename", get(internal_media_transcript_handler))
        .route("/internal/media/summary/:filename", get(internal_media_summary_handler))
        .route("/internal/media/delete/:filename", delete(internal_media_delete_handler))
        .route("/internal/assets/upload", post(internal_asset_upload_handler))
        .route("/internal/assets/:filename", get(internal_asset_view_handler))
        .with_state(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!(addr = %addr, "Starting Telemetry SSE service");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

