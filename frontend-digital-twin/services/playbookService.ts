/**
 * Playbook Service
 * 
 * Manages communication with the backend playbook API
 */

const ORCHESTRATOR_URL = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

export interface Playbook {
  id: string;
  tool_name: string;
  repository?: string;
  language?: string;
  installation_command: string;
  installation_type: string;
  verification_command?: string;
  environment_config: Record<string, string>;
  reliability_score: number;
  success_count: number;
  total_attempts: number;
  verified_by_agent?: string;
  verified_at: string;
  last_used_at?: string;
  description?: string;
  github_url?: string;
}

export interface PlaybookSearchResult {
  playbook: Playbook;
  relevance_score: number;
}

/**
 * Get all playbooks
 */
export async function getAllPlaybooks(): Promise<Playbook[]> {
  try {
    const response = await fetch(`${ORCHESTRATOR_URL}/api/playbooks`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      throw new Error(`Failed to fetch playbooks: ${response.statusText}`);
    }

    const data = await response.json();
    return data.ok ? (data.playbooks || []) : [];
  } catch (error) {
    console.error('[PlaybookService] Failed to fetch playbooks:', error);
    return [];
  }
}

/**
 * Search playbooks by tool name
 */
export async function searchPlaybooksByTool(toolName: string, limit?: number): Promise<PlaybookSearchResult[]> {
  try {
    const params = new URLSearchParams({ tool_name: toolName });
    if (limit) {
      params.append('limit', limit.toString());
    }

    const response = await fetch(`${ORCHESTRATOR_URL}/api/playbooks/search?${params}`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      throw new Error(`Failed to search playbooks: ${response.statusText}`);
    }

    const data = await response.json();
    return data.ok ? (data.results || []) : [];
  } catch (error) {
    console.error('[PlaybookService] Failed to search playbooks:', error);
    return [];
  }
}

/**
 * Search playbooks by query (semantic search)
 */
export async function searchPlaybooksByQuery(
  query: string,
  minReliability?: number,
  limit?: number
): Promise<PlaybookSearchResult[]> {
  try {
    const params = new URLSearchParams({ query });
    if (minReliability !== undefined) {
      params.append('min_reliability', minReliability.toString());
    }
    if (limit) {
      params.append('limit', limit.toString());
    }

    const response = await fetch(`${ORCHESTRATOR_URL}/api/playbooks/search?${params}`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      throw new Error(`Failed to search playbooks: ${response.statusText}`);
    }

    const data = await response.json();
    return data.ok ? (data.results || []) : [];
  } catch (error) {
    console.error('[PlaybookService] Failed to search playbooks:', error);
    return [];
  }
}

/**
 * Get top playbooks for an agent and language
 */
export async function getTopPlaybooksForAgent(
  agentId: string,
  language: string,
  limit: number = 3
): Promise<Playbook[]> {
  try {
    // Search for playbooks matching the language
    const results = await searchPlaybooksByQuery(`${language} tool`, 0.7, limit * 2);
    
    // Filter by agent if verified_by_agent is available
    const agentPlaybooks = results
      .filter(r => !r.playbook.verified_by_agent || r.playbook.verified_by_agent.includes(agentId))
      .slice(0, limit)
      .map(r => r.playbook);
    
    return agentPlaybooks;
  } catch (error) {
    console.error('[PlaybookService] Failed to get top playbooks:', error);
    return [];
  }
}

/**
 * Deploy a playbook to all agent stations in the cluster
 */
export async function deployPlaybookToCluster(playbookId: string): Promise<{
  ok: boolean;
  message?: string;
  deployment?: {
    playbook_id: string;
    playbook_name: string;
    total_agents: number;
    successful_deployments: number;
    failed_deployments: number;
    agent_results: Array<{
      agent_id: string;
      agent_name: string;
      status: string;
      message: string;
    }>;
  };
  error?: string;
}> {
  try {
    const response = await fetch(`${ORCHESTRATOR_URL}/api/playbooks/${playbookId}/deploy`, {
      method: 'POST',
      headers: {
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to deploy playbook: ${response.statusText} - ${errorText}`);
    }

    return await response.json();
  } catch (error) {
    console.error('[PlaybookService] Failed to deploy playbook:', error);
    return {
      ok: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}