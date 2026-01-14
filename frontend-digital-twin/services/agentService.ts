/**
 * Agent Service
 * Manages sub-agent (crew) operations with the orchestrator backend
 */

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

// Prefer calling the Gateway (8181) and let it proxy to the Orchestrator.
const GATEWAY_URL = import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
const ORCHESTRATOR_URL = import.meta.env.VITE_ORCHESTRATOR_URL || GATEWAY_URL;

/**
 * Fetches the list of all active sub-agents
 */
export async function listAgents(): Promise<AgentListResponse> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/agents/list`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to list agents: ${response.statusText}`);
  }

  return response.json();
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
  const response = await fetch(`${ORCHESTRATOR_URL}/api/agents/spawn`, {
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

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Failed to spawn agent: ${response.statusText} - ${errorText}`);
  }

  return response.json();
}

/**
 * Posts a task to a specific agent
 */
export async function postTaskToAgent(agentId: string, task: string): Promise<{ ok: boolean }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/agents/${agentId}/task`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      data: task,
    }),
  });

  if (!response.ok) {
    throw new Error(`Failed to post task to agent: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Gets the latest report from an agent
 */
export async function getAgentReport(agentId: string): Promise<{ ok: boolean; report: AgentReport | null }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/agents/${agentId}/report`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to get agent report: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Gets logs from an agent
 */
export async function getAgentLogs(agentId: string): Promise<{ ok: boolean; logs: string[] }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/agents/${agentId}/logs`, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to get agent logs: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Kills (terminates) an agent
 */
export async function killAgent(agentId: string): Promise<{ ok: boolean }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/agents/${agentId}/kill`, {
    method: 'POST',
    headers: {
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to kill agent: ${response.statusText}`);
  }

  return response.json();
}
