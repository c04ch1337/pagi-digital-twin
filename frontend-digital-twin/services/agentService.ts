/**
 * Agent Service
 * Manages sub-agent (crew) operations with the orchestrator backend
 */

const DEFAULT_GATEWAY_URL = 'http://127.0.0.1:8181';
const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

export interface AgentInfo {
  agent_id: string;
  name: string;
  mission: string;
  permissions: string[];
  status: string;
  created_at: string;
}

export interface AgentReport {
  agent_id: string;
  task: string;
  report: string;
  created_at: string;
}

export interface AgentListResponse {
  max_agents: number;
  agents: AgentInfo[];
}

const getGatewayUrl = (): string => {
  return (import.meta.env.VITE_GATEWAY_URL || DEFAULT_GATEWAY_URL).trim();
};

const getOrchestratorUrl = (): string => {
  // Note: unlike other services, the gateway currently does NOT proxy crew endpoints.
  // So we allow direct-to-orchestrator fallback when the gateway returns 404.
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

const truncate = (s: string, max: number): string => (s.length > max ? `${s.slice(0, max)}…` : s);

async function safeReadBody(res: Response): Promise<string> {
  try {
    const contentType = res.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
      const json = await res.json().catch(() => null);
      if (json === null) return '';
      return truncate(JSON.stringify(json), 2000);
    }
    const text = await res.text();
    return truncate(text, 2000);
  } catch {
    return '';
  }
}

async function requestAgentApi<T>(
  path: string,
  init: RequestInit
): Promise<T> {
  const gatewayUrl = getGatewayUrl();
  const orchestratorUrl = getOrchestratorUrl();

  const baseUrls = [gatewayUrl, orchestratorUrl].filter((v, i, arr) => v && arr.indexOf(v) === i);
  let lastErr: Error | null = null;

  for (let i = 0; i < baseUrls.length; i++) {
    const baseUrl = baseUrls[i];
    const url = `${baseUrl}${path}`;
    try {
      const res = await fetch(url, init);
      if (res.ok) {
        return (await res.json()) as T;
      }

      const body = await safeReadBody(res);
      const err = new Error(
        `${init.method || 'GET'} ${url} failed: ${res.status} ${res.statusText}${body ? ` - ${body}` : ''}`
      );

      // Gateway fallback: gateway doesn't currently proxy crew endpoints.
      if (baseUrl === gatewayUrl && res.status === 404 && orchestratorUrl !== gatewayUrl) {
        if (import.meta.env.DEV) {
          console.debug('[agentService] Gateway 404; falling back to orchestrator', { url, status: res.status });
        }
        lastErr = err;
        continue;
      }

      throw err;
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e));

      // Network / CORS / connection errors: fall back once (gateway -> orchestrator).
      if (baseUrl === gatewayUrl && orchestratorUrl !== gatewayUrl) {
        if (import.meta.env.DEV) {
          console.debug('[agentService] Gateway request error; falling back to orchestrator', { url, error: err.message });
        }
        lastErr = err;
        continue;
      }

      throw err;
    }
  }

  throw lastErr || new Error('Agent API request failed');
}

/**
 * Fetches the list of all active sub-agents
 */
export async function listAgents(): Promise<AgentListResponse> {
  return requestAgentApi<AgentListResponse>('/api/agents/list', {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

/**
 * Spawns a new sub-agent
 */
export async function spawnAgent(
  name: string,
  mission: string,
  permissions: string[],
  twinId: string,
  userName?: string,
  mediaActive: boolean = false
): Promise<{ ok: boolean; agent_id: string }> {
  return requestAgentApi<{ ok: boolean; agent_id: string }>('/api/agents/spawn', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      name,
      mission,
      permissions,
      twin_id: twinId,
      user_name: userName,
      media_active: mediaActive,
    }),
  });
}

/**
 * Posts a task to a specific agent
 */
export async function postTaskToAgent(agentId: string, task: string): Promise<{ ok: boolean }> {
  return requestAgentApi<{ ok: boolean }>(`/api/agents/${agentId}/task`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      data: task,
    }),
  });
}

/**
 * Gets the latest report from an agent
 */
export async function getAgentReport(agentId: string): Promise<{ ok: boolean; report: AgentReport | null }> {
  return requestAgentApi<{ ok: boolean; report: AgentReport | null }>(`/api/agents/${agentId}/report`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

/**
 * Gets logs from an agent
 */
export async function getAgentLogs(agentId: string): Promise<{ ok: boolean; logs: string[] }> {
  return requestAgentApi<{ ok: boolean; logs: string[] }>(`/api/agents/${agentId}/logs`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

/**
 * Kills (terminates) an agent
 */
export async function killAgent(agentId: string): Promise<{ ok: boolean }> {
  return requestAgentApi<{ ok: boolean }>(`/api/agents/${agentId}/kill`, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
    },
  });
}
