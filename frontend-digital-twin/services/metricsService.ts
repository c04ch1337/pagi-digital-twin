/**
 * Metrics Service
 * Fetches agent station metrics from the backend
 */

const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

const getOrchestratorUrl = (): string => {
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

export interface AgentStationMetrics {
  agent_id: string;
  agent_name: string;
  reasoning_load: number; // 0-100
  drift_frequency: number; // 0-100
  capability_score: number; // 0-100
  active_tasks: number;
  last_drift_timestamp?: string;
}

export interface MetricsStationsResponse {
  stations: AgentStationMetrics[];
}

async function requestMetricsApi<T>(path: string, init: RequestInit): Promise<T> {
  const orchestratorUrl = getOrchestratorUrl();
  const url = `${orchestratorUrl}${path}`;

  try {
    const res = await fetch(url, init);
    if (!res.ok) {
      const body = await res.text().catch(() => '');
      throw new Error(`${init.method || 'GET'} ${url} failed: ${res.status} ${res.statusText}${body ? ` - ${body}` : ''}`);
    }
    return (await res.json()) as T;
  } catch (e) {
    const err = e instanceof Error ? e : new Error(String(e));
    throw err;
  }
}

/**
 * Get metrics for all agent stations
 */
export async function getMetricsStations(): Promise<MetricsStationsResponse> {
  return requestMetricsApi<MetricsStationsResponse>('/api/phoenix/metrics/stations', {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}
