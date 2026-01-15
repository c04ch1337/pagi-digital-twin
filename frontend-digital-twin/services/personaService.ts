/**
 * Persona Service
 * Manages agent persona configuration
 */

const DEFAULT_GATEWAY_URL = 'http://127.0.0.1:8181';
const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

export interface BehavioralBias {
  cautiousness: number; // 0.0 - 1.0
  innovation: number; // 0.0 - 1.0
  detail_orientation: number; // 0.0 - 1.0
}

export interface AgentPersona {
  agent_id: string;
  name: string;
  behavioral_bias: BehavioralBias;
  voice_tone: string;
  created_at: string;
  updated_at: string;
}

export interface AssignPersonaRequest {
  agent_id: string;
  name: string;
  behavioral_bias: BehavioralBias;
  voice_tone: string;
}

const getGatewayUrl = (): string => {
  return (import.meta.env.VITE_GATEWAY_URL || DEFAULT_GATEWAY_URL).trim();
};

const getOrchestratorUrl = (): string => {
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

async function requestPersonaApi<T>(
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

      const err = new Error(
        `${init.method || 'GET'} ${url} failed: ${res.status} ${res.statusText}`
      );

      if (baseUrl === gatewayUrl && res.status === 404 && orchestratorUrl !== gatewayUrl) {
        if (import.meta.env.DEV) {
          console.debug('[personaService] Gateway 404; falling back to orchestrator', { url, status: res.status });
        }
        lastErr = err;
        continue;
      }

      throw err;
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e));

      if (baseUrl === gatewayUrl && orchestratorUrl !== gatewayUrl) {
        if (import.meta.env.DEV) {
          console.debug('[personaService] Gateway request error; falling back to orchestrator', { url, error: err.message });
        }
        lastErr = err;
        continue;
      }

      throw err;
    }
  }

  throw lastErr || new Error('Persona API request failed');
}

/**
 * Get persona for an agent
 */
export async function getPersona(agentId: string): Promise<{ success: boolean; persona: AgentPersona | null }> {
  return requestPersonaApi<{ success: boolean; persona: AgentPersona | null }>(`/api/agents/${agentId}/persona`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

/**
 * Assign persona to an agent
 */
export async function assignPersona(request: AssignPersonaRequest): Promise<{ success: boolean; message: string }> {
  return requestPersonaApi<{ success: boolean; message: string }>('/api/agents/persona', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(request),
  });
}

/**
 * Get all personas
 */
export async function getAllPersonas(): Promise<{ success: boolean; personas: AgentPersona[] }> {
  return requestPersonaApi<{ success: boolean; personas: AgentPersona[] }>('/api/agents/personas', {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}
