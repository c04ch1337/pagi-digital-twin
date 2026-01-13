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

export interface SyncMetricsResponse {
  neural_sync: number;
  services: Record<string, string>;
}

// Prefer calling the Gateway (8181) and let it proxy to the Orchestrator.
// This avoids CORS/config drift across multiple frontend services.
//
// Override via:
// - VITE_GATEWAY_URL (recommended) e.g. http://127.0.0.1:8181
// - VITE_ORCHESTRATOR_URL (advanced) e.g. http://127.0.0.1:8182
const GATEWAY_URL = import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
const ORCHESTRATOR_URL = import.meta.env.VITE_ORCHESTRATOR_URL || GATEWAY_URL;

/**
 * Fetches the current system snapshot from the orchestrator
 */
export async function fetchSystemSnapshot(): Promise<SystemSnapshot> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/system/snapshot`, {
    method: 'GET',
    // IMPORTANT: Do NOT set `Content-Type: application/json` on a GET.
    // That header is not "simple" and forces a CORS preflight (OPTIONS).
    // The gateway/orchestrator path should be a simple GET.
    headers: {
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch system snapshot: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Fetches sync metrics (neural_sync + per-service status) from the orchestrator.
 */
export async function fetchSyncMetrics(): Promise<SyncMetricsResponse> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/system/sync-metrics`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to fetch sync metrics: ${response.statusText}`);
  }

  return response.json();
}
