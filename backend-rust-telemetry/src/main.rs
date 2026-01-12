use axum::{
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Router,
};
use futures_core::stream::Stream;
use serde::Serialize;
use std::{
    convert::Infallible,
    env,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tracing::info;

#[derive(Debug, Serialize)]
struct TelemetryPayload {
    ts_ms: u128,
    cpu_percent: f32,
    mem_total: u64,
    mem_used: u64,
    mem_free: u64,
    process_count: usize,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis()
}

async fn sse_stream() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
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

    let stream = async_stream::stream! {
        loop {
            ticker.tick().await;

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
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)).text("keep-alive"))
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

    let app = Router::new().route("/v1/telemetry/stream", get(sse_stream));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!(addr = %addr, "Starting Telemetry SSE service");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

