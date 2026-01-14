/**
 * Service for managing environment variables (.env file) via the orchestrator API
 */

export interface EnvReadResponse {
  env_vars: Record<string, string>;
  env_file_path: string;
}

export interface EnvUpdateRequest {
  updates: Record<string, string>;
}

export interface EnvUpdateResponse {
  success: boolean;
  message: string;
}

const getOrchestratorUrl = (): string => {
  return (
    localStorage.getItem('root_admin_orchestrator_url') ||
    import.meta.env.VITE_ORCHESTRATOR_URL ||
    'http://127.0.0.1:8182'
  );
};

const getGatewayUrl = (): string => {
  return (
    localStorage.getItem('root_admin_gateway_url') ||
    import.meta.env.VITE_GATEWAY_URL ||
    'http://127.0.0.1:8181'
  );
};

/**
 * Reads the current .env file from the server
 */
export async function readEnvFile(): Promise<EnvReadResponse> {
  const gatewayUrl = getGatewayUrl();
  const orchestratorUrl = getOrchestratorUrl();

  // Try gateway first, then orchestrator directly
  let response = await fetch(`${gatewayUrl}/api/env/read`, {
    method: 'GET',
    headers: { Accept: 'application/json' },
  });

  if (!response.ok && response.status === 404) {
    response = await fetch(`${orchestratorUrl}/api/env/read`, {
      method: 'GET',
      headers: { Accept: 'application/json' },
    });
  }

  if (!response.ok) {
    throw new Error(`Failed to read .env file: ${response.status} ${response.statusText}`);
  }

  return response.json();
}

/**
 * Updates the .env file on the server
 */
export async function updateEnvFile(updates: Record<string, string>): Promise<EnvUpdateResponse> {
  const gatewayUrl = getGatewayUrl();
  const orchestratorUrl = getOrchestratorUrl();

  const request: EnvUpdateRequest = { updates };

  // Try gateway first, then orchestrator directly
  let response = await fetch(`${gatewayUrl}/api/env/update`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });

  if (!response.ok && response.status === 404) {
    response = await fetch(`${orchestratorUrl}/api/env/update`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
  }

  if (!response.ok) {
    throw new Error(`Failed to update .env file: ${response.status} ${response.statusText}`);
  }

  return response.json();
}

/**
 * Security gate names that can be toggled
 */
export const SECURITY_GATES = [
  {
    key: 'ALLOW_PUBLIC_NETWORK_SCAN',
    label: 'Allow Public Network Scan',
    description: 'Enable network scanning of public IP addresses (requires NETWORK_SCAN_HITL_TOKEN)',
    category: 'network',
  },
  {
    key: 'ALLOW_IPV6_NETWORK_SCAN',
    label: 'Allow IPv6 Network Scan',
    description: 'Enable IPv6 network scanning support',
    category: 'network',
  },
  {
    key: 'ALLOW_ARBITRARY_PORT_SCAN',
    label: 'Allow Arbitrary Port Scan',
    description: 'Allow custom port ranges for network scanning (not just ports 8281-8284)',
    category: 'network',
  },
  {
    key: 'BYPASS_HITL_TOOL_EXEC',
    label: 'Bypass HITL for Tool Execution',
    description: 'Bypass human approval requirement for tool_exec actions (CRITICAL: removes safety check)',
    category: 'hitl',
  },
  {
    key: 'BYPASS_HITL_MEMORY',
    label: 'Bypass HITL for Memory Operations',
    description: 'Bypass human approval requirement for memory_query and memory_commit actions',
    category: 'hitl',
  },
  {
    key: 'BYPASS_HITL_KILL_PROCESS',
    label: 'Bypass HITL for Process Termination',
    description: 'Bypass human approval requirement for kill_process actions (CRITICAL: allows terminating any process)',
    category: 'hitl',
  },
  {
    key: 'BYPASS_EMAIL_TEAMS_APPROVAL',
    label: 'Bypass Email/Teams Approval',
    description: 'Bypass user approval requirement for sending emails and Teams messages (CRITICAL: allows sending on your behalf)',
    category: 'communication',
  },
  {
    key: 'ALLOW_RESTRICTED_COMMANDS',
    label: 'Allow Restricted Commands',
    description: 'Allow execution of restricted commands (rm, delete, format, shutdown, reboot) (CRITICAL: can cause data loss)',
    category: 'commands',
  },
] as const;

/**
 * Helper to check if a security gate is enabled
 */
export function isSecurityGateEnabled(envVars: Record<string, string>, gateKey: string): boolean {
  const value = envVars[gateKey]?.toLowerCase().trim();
  return value === '1' || value === 'true' || value === 'yes' || value === 'on';
}

/**
 * Helper to set a security gate value
 */
export function setSecurityGateValue(enabled: boolean): string {
  return enabled ? '1' : '0';
}
