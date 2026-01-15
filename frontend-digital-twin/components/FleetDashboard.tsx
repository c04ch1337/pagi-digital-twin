import React, { useState, useEffect, useCallback } from 'react';

interface FleetDashboardProps {
  onClose?: () => void;
}

interface Node {
  node_id: string;
  hostname: string;
  ip_address: string;
  status: 'nominal' | 'in_drift' | 'in_repair' | 'stale' | 'offline';
  last_heartbeat: string;
  last_audit_timestamp: string | null;
  software_version: string | null;
  registered_at: string;
}

interface FleetHealth {
  total_nodes: number;
  nominal: number;
  in_drift: number;
  in_repair: number;
  stale: number;
  offline: number;
}

interface FleetStatus {
  nodes: Node[];
  health: FleetHealth;
}

const FleetDashboard: React.FC<FleetDashboardProps> = ({ onClose }) => {
  const [fleetStatus, setFleetStatus] = useState<FleetStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<string | null>(null);

  const loadFleetStatus = useCallback(async () => {
    try {
      setError(null);
      const response = await fetch('/api/fleet/status');
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }
      const data: FleetStatus = await response.json();
      setFleetStatus(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load fleet status');
      console.error('Failed to fetch fleet status:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadFleetStatus();
    // Poll every 5 seconds
    const interval = setInterval(loadFleetStatus, 5000);
    return () => clearInterval(interval);
  }, [loadFleetStatus]);

  const getStatusColor = (status: Node['status']): string => {
    switch (status) {
      case 'nominal':
        return 'bg-[var(--success)]';
      case 'in_drift':
        return 'bg-[rgb(var(--warning-rgb))]';
      case 'in_repair':
        return 'bg-[rgb(var(--warning-rgb))]';
      case 'stale':
        return 'bg-[rgb(var(--text-secondary-rgb)/0.5)]';
      case 'offline':
        return 'bg-[rgb(var(--error-rgb))]';
      default:
        return 'bg-[rgb(var(--text-secondary-rgb)/0.35)]';
    }
  };

  const getStatusLabel = (status: Node['status']): string => {
    switch (status) {
      case 'nominal':
        return 'Nominal';
      case 'in_drift':
        return 'In Drift';
      case 'in_repair':
        return 'In Repair';
      case 'stale':
        return 'Stale';
      case 'offline':
        return 'Offline';
      default:
        return 'Unknown';
    }
  };

  const formatTimestamp = (timestamp: string): string => {
    try {
      const date = new Date(timestamp);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffSecs = Math.floor(diffMs / 1000);
      
      if (diffSecs < 60) {
        return `${diffSecs}s ago`;
      } else if (diffSecs < 3600) {
        const mins = Math.floor(diffSecs / 60);
        return `${mins}m ago`;
      } else if (diffSecs < 86400) {
        const hours = Math.floor(diffSecs / 3600);
        return `${hours}h ago`;
      } else {
        return date.toLocaleDateString();
      }
    } catch {
      return 'Unknown';
    }
  };

  const selectedNodeData = fleetStatus?.nodes.find(n => n.node_id === selectedNode);

  return (
    <div className="flex-1 flex flex-col bg-[var(--bg-primary)] overflow-hidden relative">
      {/* Background Tactical Grid */}
      <div
        className="absolute inset-0 opacity-[0.03] pointer-events-none"
        style={{
          backgroundImage:
            'linear-gradient(rgb(var(--bg-steel-rgb)) 1px, transparent 1px), linear-gradient(90deg, rgb(var(--bg-steel-rgb)) 1px, transparent 1px)',
          backgroundSize: '40px 40px',
        }}
      />

      {/* Top rail */}
      <div className="relative z-10 flex items-center justify-between gap-3 px-4 py-3 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">hub</span>
          <div className="min-w-0">
            <div className="text-[11px] font-black uppercase tracking-tight text-[var(--text-primary)]">
              Phoenix Fleet Manager
            </div>
            <div className="text-[9px] text-[var(--text-secondary)] font-bold uppercase tracking-widest truncate">
              Distributed node registry & health monitoring
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {onClose && (
            <button
              onClick={onClose}
              className="px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]"
            >
              <span className="material-symbols-outlined text-[14px] align-middle mr-1">close</span>
              Close
            </button>
          )}
          <button
            onClick={loadFleetStatus}
            className="px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]"
            disabled={loading}
          >
            <span className="material-symbols-outlined text-[14px] align-middle mr-1">refresh</span>
            Refresh
          </button>
        </div>
      </div>

      {/* Main content */}
      <div className="relative z-10 flex-1 min-h-0 p-4 overflow-auto">
        {loading && !fleetStatus ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-[var(--text-secondary)]">Loading fleet status...</div>
          </div>
        ) : error ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-[rgb(var(--error-rgb))]">Error: {error}</div>
          </div>
        ) : !fleetStatus ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-[var(--text-secondary)]">No fleet data available</div>
          </div>
        ) : (
          <div className="grid grid-cols-1 lg:grid-cols-[300px_1fr] gap-4 h-full">
            {/* Left sidebar: Health summary */}
            <div className="space-y-4">
              {/* Fleet Health Summary */}
              <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4">
                <div className="flex items-center gap-2 mb-3">
                  <span className="material-symbols-outlined text-[var(--bg-steel)]">monitoring</span>
                  <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                    Fleet Health
                  </h3>
                </div>

                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-[var(--text-secondary)]">Total Nodes</span>
                    <span className="text-[11px] font-bold text-[var(--text-primary)]">
                      {fleetStatus.health.total_nodes}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-[var(--text-secondary)] flex items-center gap-1">
                      <span className="w-2 h-2 rounded-full bg-[var(--success)]"></span>
                      Nominal
                    </span>
                    <span className="text-[11px] font-bold text-[var(--text-primary)]">
                      {fleetStatus.health.nominal}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-[var(--text-secondary)] flex items-center gap-1">
                      <span className="w-2 h-2 rounded-full bg-[rgb(var(--warning-rgb))]"></span>
                      In Drift
                    </span>
                    <span className="text-[11px] font-bold text-[var(--text-primary)]">
                      {fleetStatus.health.in_drift}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-[var(--text-secondary)] flex items-center gap-1">
                      <span className="w-2 h-2 rounded-full bg-[rgb(var(--warning-rgb))]" style={{ animation: 'pulse-glow 2s ease-in-out infinite' }}></span>
                      In Repair
                    </span>
                    <span className="text-[11px] font-bold text-[var(--text-primary)]">
                      {fleetStatus.health.in_repair}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-[var(--text-secondary)] flex items-center gap-1">
                      <span className="w-2 h-2 rounded-full bg-[rgb(var(--text-secondary-rgb)/0.5)]"></span>
                      Stale
                    </span>
                    <span className="text-[11px] font-bold text-[var(--text-primary)]">
                      {fleetStatus.health.stale}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-[var(--text-secondary)] flex items-center gap-1">
                      <span className="w-2 h-2 rounded-full bg-[rgb(var(--error-rgb))]"></span>
                      Offline
                    </span>
                    <span className="text-[11px] font-bold text-[var(--text-primary)]">
                      {fleetStatus.health.offline}
                    </span>
                  </div>
                </div>
              </div>

              {/* Node Details (if selected) */}
              {selectedNodeData && (
                <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4">
                  <div className="flex items-center justify-between mb-3">
                    <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                      Node Details
                    </h3>
                    <button
                      onClick={() => setSelectedNode(null)}
                      className="text-[10px] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                    >
                      <span className="material-symbols-outlined text-sm">close</span>
                    </button>
                  </div>

                  <div className="space-y-2 text-[10px]">
                    <div>
                      <span className="text-[var(--text-secondary)]">Hostname:</span>
                      <div className="text-[var(--text-primary)] font-mono">{selectedNodeData.hostname}</div>
                    </div>
                    <div>
                      <span className="text-[var(--text-secondary)]">IP Address:</span>
                      <div className="text-[var(--text-primary)] font-mono">{selectedNodeData.ip_address}</div>
                    </div>
                    <div>
                      <span className="text-[var(--text-secondary)]">Node ID:</span>
                      <div className="text-[var(--text-primary)] font-mono truncate">{selectedNodeData.node_id}</div>
                    </div>
                    {selectedNodeData.software_version && (
                      <div>
                        <span className="text-[var(--text-secondary)]">Version:</span>
                        <div className="text-[var(--text-primary)]">{selectedNodeData.software_version}</div>
                      </div>
                    )}
                    <div>
                      <span className="text-[var(--text-secondary)]">Last Heartbeat:</span>
                      <div className="text-[var(--text-primary)]">{formatTimestamp(selectedNodeData.last_heartbeat)}</div>
                    </div>
                    {selectedNodeData.last_audit_timestamp && (
                      <div>
                        <span className="text-[var(--text-secondary)]">Last Audit:</span>
                        <div className="text-[var(--text-primary)]">{formatTimestamp(selectedNodeData.last_audit_timestamp)}</div>
                      </div>
                    )}
                    <div>
                      <span className="text-[var(--text-secondary)]">Registered:</span>
                      <div className="text-[var(--text-primary)]">{formatTimestamp(selectedNodeData.registered_at)}</div>
                    </div>
                    {selectedNodeData.status === 'in_repair' && (
                      <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
                        <a
                          href={`/audit?node=${selectedNodeData.node_id}`}
                          className="text-[10px] text-[var(--bg-steel)] hover:underline flex items-center gap-1"
                        >
                          <span className="material-symbols-outlined text-xs">open_in_new</span>
                          View Audit Logs
                        </a>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>

            {/* Main area: Node grid */}
            <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 overflow-auto">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                  Node Map ({fleetStatus.nodes.length} nodes)
                </h3>
              </div>

              {fleetStatus.nodes.length === 0 ? (
                <div className="text-center py-8 text-[var(--text-secondary)]">
                  No nodes registered in the fleet
                </div>
              ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                  {fleetStatus.nodes.map((node) => (
                    <div
                      key={node.node_id}
                      onClick={() => setSelectedNode(node.node_id)}
                      className={`bg-[rgb(var(--bg-secondary-rgb)/0.4)] border rounded-lg p-3 cursor-pointer transition-all hover:bg-[rgb(var(--bg-secondary-rgb)/0.6)] ${
                        selectedNode === node.node_id
                          ? 'border-[var(--bg-steel)] ring-2 ring-[var(--bg-steel)]'
                          : 'border-[rgb(var(--bg-steel-rgb)/0.3)]'
                      }`}
                    >
                      <div className="flex items-start justify-between mb-2">
                        <div className="flex-1 min-w-0">
                          <div className="text-[11px] font-bold text-[var(--text-primary)] truncate">
                            {node.hostname}
                          </div>
                          <div className="text-[9px] text-[var(--text-secondary)] font-mono truncate">
                            {node.ip_address}
                          </div>
                        </div>
                        <div
                          className={`w-3 h-3 rounded-full flex-shrink-0 ${getStatusColor(node.status)} ${
                            node.status === 'in_repair' ? 'animate-pulse' : ''
                          }`}
                          title={getStatusLabel(node.status)}
                        />
                      </div>

                      <div className="space-y-1 text-[9px]">
                        <div className="flex items-center justify-between">
                          <span className="text-[var(--text-secondary)]">Status:</span>
                          <span className={`font-bold ${
                            node.status === 'nominal' ? 'text-[var(--success)]' :
                            node.status === 'in_repair' || node.status === 'in_drift' ? 'text-[rgb(var(--warning-rgb))]' :
                            node.status === 'offline' ? 'text-[rgb(var(--error-rgb))]' :
                            'text-[var(--text-secondary)]'
                          }`}>
                            {getStatusLabel(node.status)}
                          </span>
                        </div>
                        <div className="flex items-center justify-between">
                          <span className="text-[var(--text-secondary)]">Heartbeat:</span>
                          <span className="text-[var(--text-primary)]">{formatTimestamp(node.last_heartbeat)}</span>
                        </div>
                        {node.last_audit_timestamp && (
                          <div className="flex items-center justify-between">
                            <span className="text-[var(--text-secondary)]">Last Audit:</span>
                            <span className="text-[var(--text-primary)]">{formatTimestamp(node.last_audit_timestamp)}</span>
                          </div>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default FleetDashboard;
