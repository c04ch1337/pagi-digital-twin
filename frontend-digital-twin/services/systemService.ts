/**
 * System Service
 * Fetches system snapshot data from the orchestrator backend
 */

export interface ProcessSnapshot {
  pid: number;
  name: string;
  memory_kib: number;
}

export interface CpuSnapshot {
  global_usage_percent: number;
  per_core_usage_percent: number[];
}

export interface MemorySnapshot {
  total_kib: number;
  used_kib: number;
}

export interface SystemSnapshot {
  memory: MemorySnapshot;
  cpu: CpuSnapshot;
  top_processes: ProcessSnapshot[];
}

const ORCHESTRATOR_URL = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

/**
 * Fetches the current system snapshot from the orchestrator
 */
export async function fetchSystemSnapshot(): Promise<SystemSnapshot> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/system/snapshot`, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch system snapshot: ${response.statusText}`);
  }

  return response.json();
}
