const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

const getOrchestratorUrl = (): string => {
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

export interface RetrospectiveAnalysis {
  retrospective_id: string;
  playbook_id: string;
  tool_name: string;
  failure_timestamp: string;
  agent_id: string;
  agent_name: string;
  root_cause: string;
  error_pattern: string;
  expected_verification: string;
  actual_error_output: string;
  suggested_patch?: {
    patch_id: string;
    playbook_id: string;
    original_command: string;
    patched_command: string;
    patch_reason: string;
    confidence: number;
    created_at: string;
  };
  reliability_impact: number;
  created_at: string;
}

async function retrospectiveApi<T>(path: string, init: RequestInit): Promise<T> {
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

export async function getAllRetrospectives(): Promise<{ ok: boolean; retrospectives: RetrospectiveAnalysis[]; count: number }> {
  return retrospectiveApi<{ ok: boolean; retrospectives: RetrospectiveAnalysis[]; count: number }>('/api/retrospectives', {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

export async function getRetrospectivesForPlaybook(playbookId: string): Promise<{ ok: boolean; playbook_id: string; retrospectives: RetrospectiveAnalysis[]; count: number }> {
  return retrospectiveApi<{ ok: boolean; playbook_id: string; retrospectives: RetrospectiveAnalysis[]; count: number }>(`/api/retrospectives/playbook/${playbookId}`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

export async function getRetrospective(retrospectiveId: string): Promise<{ ok: boolean; retrospective?: RetrospectiveAnalysis; error?: string }> {
  return retrospectiveApi<{ ok: boolean; retrospective?: RetrospectiveAnalysis; error?: string }>(`/api/retrospectives/${retrospectiveId}`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

export async function applyPatch(retrospectiveId: string): Promise<{ ok: boolean; message: string; playbook_id?: string }> {
  return retrospectiveApi<{ ok: boolean; message: string; playbook_id?: string }>(`/api/retrospectives/${retrospectiveId}/apply-patch`, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/json',
    },
  });
}
