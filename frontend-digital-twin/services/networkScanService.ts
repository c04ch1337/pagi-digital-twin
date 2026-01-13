/**
 * Network Scan Service
 * Calls the Orchestrator network scan endpoints.
 *
 * Notes:
 * - Prefer calling the Gateway (8181) and let it proxy to the Orchestrator.
 * - If the gateway doesn't have the route (404), fall back to the Orchestrator directly.
 */

import type { NetworkScanResult } from '../types/networkScan';

const GATEWAY_URL = import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
const ORCHESTRATOR_URL = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

async function fetchJsonWithFallback(inputPath: string, init: RequestInit): Promise<Response> {
  const gatewayUrl = `${GATEWAY_URL}${inputPath}`;
  let resp = await fetch(gatewayUrl, init);

  // If gateway doesn't have the route, try orchestrator directly.
  if (!resp.ok && resp.status === 404) {
    resp = await fetch(`${ORCHESTRATOR_URL}${inputPath}`, init);
  }

  return resp;
}

export async function runNetworkScan(params: {
  target: string;
  twin_id: string;
  namespace?: string;
  hitl_token?: string;
}): Promise<NetworkScanResult> {
  const response = await fetchJsonWithFallback('/api/network/scan', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      target: params.target,
      twin_id: params.twin_id,
      namespace: params.namespace || 'default',
      hitl_token: params.hitl_token,
    }),
  });

  if (!response.ok) {
    const text = await response.text().catch(() => '');
    throw new Error(`Network scan failed: ${response.status} ${response.statusText}${text ? ` - ${text}` : ''}`);
  }

  return response.json();
}

export async function fetchLatestNetworkScan(params: {
  twin_id: string;
  namespace?: string;
}): Promise<NetworkScanResult | null> {
  const ns = params.namespace || 'default';
  const query = new URLSearchParams({ twin_id: params.twin_id, namespace: ns }).toString();
  const response = await fetchJsonWithFallback(`/api/network/scan/latest?${query}`, {
    method: 'GET',
    headers: { Accept: 'application/json' },
  });

  if (response.status === 404) return null;
  if (!response.ok) {
    throw new Error(`Failed to fetch network scan: ${response.status} ${response.statusText}`);
  }

  return response.json();
}


