use serde::Serialize;
use sysinfo::{ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize)]
pub struct MemorySnapshot {
    /// Total physical memory (KiB per sysinfo).
    pub total_kib: u64,
    /// Used physical memory (KiB per sysinfo).
    pub used_kib: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuSnapshot {
    /// Global CPU usage percentage (0-100).
    pub global_usage_percent: f32,
    /// Per-core CPU usage percentage (0-100).
    pub per_core_usage_percent: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    /// Resident memory (KiB per sysinfo).
    pub memory_kib: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub memory: MemorySnapshot,
    pub cpu: CpuSnapshot,
    /// Top processes by memory usage, descending.
    pub top_processes: Vec<ProcessSnapshot>,
}

fn pid_to_u32_lossy(pid: impl std::fmt::Display) -> u32 {
    pid.to_string().parse::<u32>().unwrap_or(0)
}

/// Cross-platform system monitor.
///
/// Notes:
/// - sysinfo CPU usage requires two refreshes separated by a small delay to produce non-zero values.
/// - sysinfo memory/process units are reported in KiB.
pub async fn get_system_snapshot() -> Result<SystemSnapshot, String> {
    let mut sys = System::new_all();

    // Populate initial values.
    sys.refresh_all();

    // CPU usage is computed between refreshes.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    sys.refresh_cpu_all();

    // Ensure memory/processes are up to date.
    sys.refresh_memory();
    // Refresh all processes and remove dead ones.
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let total_kib = sys.total_memory();
    let used_kib = sys.used_memory();

    let global_usage_percent = sys.global_cpu_usage();
    let per_core_usage_percent = sys.cpus().iter().map(|c| c.cpu_usage()).collect();

    let mut procs: Vec<ProcessSnapshot> = sys
        .processes()
        .values()
        .map(|p| ProcessSnapshot {
            pid: pid_to_u32_lossy(p.pid()),
            name: p.name().to_string_lossy().to_string(),
            memory_kib: p.memory(),
        })
        .collect();

    procs.sort_by(|a, b| b.memory_kib.cmp(&a.memory_kib));
    procs.truncate(10);

    Ok(SystemSnapshot {
        memory: MemorySnapshot { total_kib, used_kib },
        cpu: CpuSnapshot {
            global_usage_percent,
            per_core_usage_percent,
        },
        top_processes: procs,
    })
}
