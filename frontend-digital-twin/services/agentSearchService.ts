/**
 * Agent Search Service
 * Semantic search for agents using Qdrant
 */

const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

export interface AgentSearchResult {
  agent_id: string;
  agent_name: string;
  mission: string;
  score: number;
  status: string;
}

export interface AgentSearchResponse {
  results: AgentSearchResult[];
}

const getOrchestratorUrl = (): string => {
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

export async function searchAgents(query: string, topK: number = 5): Promise<AgentSearchResult[]> {
  const orchestratorUrl = getOrchestratorUrl();
  const url = `${orchestratorUrl}/api/agents/search?query=${encodeURIComponent(query)}&top_k=${topK}`;

  const res = await fetch(url, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
  });

  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`GET ${url} failed: ${res.status} ${res.statusText}${body ? ` - ${body}` : ''}`);
  }

  const response = (await res.json()) as AgentSearchResponse;
  return response.results;
}
