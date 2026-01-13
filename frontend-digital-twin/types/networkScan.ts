/**
 * Network scan types (Orchestrator → UI contract)
 *
 * Mirrors Rust structs:
 * - NetworkScanResult / NetworkScanHost / NetworkScanPort
 *
 * See [`NetworkScanResult`](backend-rust-orchestrator/src/main.rs:726).
 */

export interface NetworkScanPort {
  port: number;
  protocol: string;
  state: string;
  service?: string;
}

export interface NetworkScanHost {
  ipv4?: string;
  hostnames: string[];
  ports: NetworkScanPort[];
  is_agi_core_node: boolean;
}

export interface NetworkScanResult {
  target: string;
  timestamp: string;
  scanned_ports: number[];
  hosts: NetworkScanHost[];
}

