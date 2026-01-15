/**
 * Tool Proposal Service
 * Manages tool installation proposals from agents (e.g., Phoenix Auditor)
 */

const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

const getOrchestratorUrl = (): string => {
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

export interface RepairProposal {
  tool_name: string;
  installation_command: string;
  rollback_command: string;
  last_successful_timestamp?: string;
  last_successful_command?: string;
  repair_reason: string;
  confidence: number;
}

export interface ToolInstallationProposal {
  id: string;
  agent_id: string;
  agent_name: string;
  repository: string;
  tool_name: string;
  description: string;
  github_url: string;
  stars: number;
  language?: string;
  installation_command: string;
  code_snippet: string;
  status: 'pending' | 'approved' | 'rejected';
  created_at: string;
  reviewed_at?: string;
  installation_success?: boolean;
  verified?: boolean;
  verification_message?: string;
  repair_proposal?: RepairProposal;
}

export interface CreateToolProposalRequest {
  agent_id: string;
  agent_name: string;
  repository: string;
  tool_name: string;
  description: string;
  github_url: string;
  stars: number;
  language?: string;
  installation_command: string;
  code_snippet: string;
}

export interface ToolProposalsResponse {
  proposals: ToolInstallationProposal[];
}

async function requestToolProposalApi<T>(
  path: string,
  init: RequestInit
): Promise<T> {
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
 * Get all tool installation proposals
 */
export async function getToolProposals(): Promise<ToolProposalsResponse> {
  return requestToolProposalApi<ToolProposalsResponse>('/api/tool-proposals', {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

/**
 * Get pending tool installation proposals
 */
export async function getPendingToolProposals(): Promise<ToolProposalsResponse> {
  return requestToolProposalApi<ToolProposalsResponse>('/api/tool-proposals/pending', {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });
}

/**
 * Create a new tool installation proposal
 */
export async function createToolProposal(
  request: CreateToolProposalRequest
): Promise<{ ok: boolean; proposal_id: string; proposal: ToolInstallationProposal }> {
  return requestToolProposalApi<{ ok: boolean; proposal_id: string; proposal: ToolInstallationProposal }>(
    '/api/tool-proposals',
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify(request),
    }
  );
}

/**
 * Approve a tool installation proposal
 */
export async function approveToolProposal(proposalId: string): Promise<{ ok: boolean; message: string }> {
  return requestToolProposalApi<{ ok: boolean; message: string }>(
    `/api/tool-proposals/${proposalId}/approve`,
    {
      method: 'POST',
      headers: {
        Accept: 'application/json',
      },
    }
  );
}

/**
 * Reject a tool installation proposal
 */
export async function rejectToolProposal(proposalId: string): Promise<{ ok: boolean; message: string }> {
  return requestToolProposalApi<{ ok: boolean; message: string }>(
    `/api/tool-proposals/${proposalId}/reject`,
    {
      method: 'POST',
      headers: {
        Accept: 'application/json',
      },
    }
  );
}

/**
 * Simulation result structure
 */
export interface SimulationResult {
  success: boolean;
  message: string;
  installation_output: string;
  verification_output: string;
  sandbox_path: string;
  errors: string[];
}

/**
 * Simulate a tool installation in a sandbox
 */
export async function simulateToolProposal(proposalId: string): Promise<{ 
  ok: boolean; 
  simulation?: SimulationResult;
  message?: string;
  error?: string;
}> {
  return requestToolProposalApi<{ 
    ok: boolean; 
    simulation?: SimulationResult;
    message?: string;
    error?: string;
  }>(
    `/api/tool-proposals/${proposalId}/simulate`,
    {
      method: 'POST',
      headers: {
        Accept: 'application/json',
      },
    }
  );
}