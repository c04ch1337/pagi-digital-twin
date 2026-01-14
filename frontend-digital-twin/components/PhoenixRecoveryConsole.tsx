import React, { useState, useEffect, useRef } from 'react';
import { formatCompactBytes } from '../utils/formatMetrics';

interface SnapshotInfo {
  snapshot_id: string;
  collection_name: string;
  creation_time: string;
  size: number;
  compliance_score?: number;
  is_recommended: boolean;
  is_blessed: boolean;
}

interface PhoenixRecoveryConsoleProps {
  onClose?: () => void;
}

const PhoenixRecoveryConsole: React.FC<PhoenixRecoveryConsoleProps> = ({ onClose }) => {
  const [snapshots, setSnapshots] = useState<SnapshotInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [restoring, setRestoring] = useState<string | null>(null);
  const [maintenanceMode, setMaintenanceMode] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Fetch snapshots
  const fetchSnapshots = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch('/api/phoenix/memory/snapshots');
      if (!response.ok) {
        throw new Error('Failed to fetch snapshots');
      }
      const data = await response.json();
      setSnapshots(data.snapshots || []);
    } catch (err) {
      console.error('[PhoenixRecoveryConsole] Failed to fetch snapshots:', err);
      setError(err instanceof Error ? err.message : 'Failed to load snapshots');
    } finally {
      setLoading(false);
    }
  };

  // Fetch maintenance mode status
  const fetchMaintenanceMode = async () => {
    try {
      const response = await fetch('/api/phoenix/maintenance/status');
      if (response.ok) {
        const data = await response.json();
        setMaintenanceMode(data.enabled || false);
      }
    } catch (err) {
      console.error('[PhoenixRecoveryConsole] Failed to fetch maintenance mode:', err);
    }
  };

  useEffect(() => {
    fetchSnapshots();
    fetchMaintenanceMode();
    
    // Poll maintenance mode status every 2 seconds
    const interval = setInterval(fetchMaintenanceMode, 2000);
    return () => clearInterval(interval);
  }, []);

  // Handle Esc key with confirmation
  useEffect(() => {
    if (!onClose) return;
    
    const handleEscKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !restoring) {
        if (confirm('Close Recovery Console?')) {
          onClose();
        }
      }
    };

    window.addEventListener('keydown', handleEscKey);
    return () => window.removeEventListener('keydown', handleEscKey);
  }, [onClose, restoring]);

  // Lock background interactions when modal-like
  useEffect(() => {
    if (!onClose) return; // Only lock if it's a modal (has onClose)
    
    const appContainer = document.querySelector('[data-app-container]') || document.body;
    const originalPointerEvents = (appContainer as HTMLElement).style.pointerEvents;
    (appContainer as HTMLElement).style.pointerEvents = 'none';
    
    // Ensure console itself is interactive
    if (containerRef.current) {
      containerRef.current.style.pointerEvents = 'auto';
    }

    return () => {
      (appContainer as HTMLElement).style.pointerEvents = originalPointerEvents || '';
    };
  }, [onClose]);

  // Filter snapshots based on search query
  const filteredSnapshots = snapshots.filter(snapshot =>
    snapshot.snapshot_id.toLowerCase().includes(searchQuery.toLowerCase()) ||
    snapshot.collection_name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // Restore snapshot
  const handleRestore = async (snapshot: SnapshotInfo) => {
    if (maintenanceMode) {
      alert('System is currently in maintenance mode. Please wait for the current operation to complete.');
      return;
    }

    if (!confirm(
      `Are you sure you want to restore from snapshot "${snapshot.snapshot_id}"?\n\n` +
      `Collection: ${snapshot.collection_name}\n` +
      `Created: ${formatTimestamp(snapshot.creation_time)}\n\n` +
      `This will:\n` +
      `1. Enable maintenance mode (pause all PhoenixEvent bus traffic)\n` +
      `2. Restore the collection from this snapshot\n` +
      `3. Disable maintenance mode after completion\n\n` +
      `This operation cannot be undone.`
    )) {
      return;
    }

    setRestoring(snapshot.snapshot_id);
    try {
      const response = await fetch('/api/phoenix/memory/restore', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          snapshot_id: snapshot.snapshot_id,
          collection_name: snapshot.collection_name,
        }),
      });

      const data = await response.json();
      if (data.success) {
        alert(`Snapshot restored successfully!\n\n${data.message}`);
        // Refresh snapshots after restore
        await fetchSnapshots();
        await fetchMaintenanceMode();
      } else {
        alert(`Failed to restore snapshot:\n\n${data.message}`);
      }
    } catch (err) {
      console.error('[PhoenixRecoveryConsole] Failed to restore snapshot:', err);
      alert('Failed to restore snapshot. Please try again.');
    } finally {
      setRestoring(null);
    }
  };

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  const formatSize = (bytes: number) => formatCompactBytes(bytes);

  return (
    <div 
      ref={containerRef}
      className="flex-1 flex flex-col bg-[var(--bg-primary)] overflow-hidden font-display text-[var(--text-primary)]"
      data-phoenix-modal={onClose ? "true" : undefined}
      style={onClose ? {
        border: '2px solid rgb(var(--bg-steel-rgb))',
        boxShadow: '0 0 30px rgba(var(--bg-steel-rgb), 0.6), 0 0 60px rgba(var(--bg-steel-rgb), 0.3), inset 0 0 20px rgba(var(--bg-steel-rgb), 0.1)',
        animation: 'phoenix-glow 3s ease-in-out infinite',
      } : undefined}
    >
      {/* Header */}
      <div className="p-6 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-[var(--bg-steel)]">restore</span>
            <h2 className="text-xl font-bold text-[var(--text-primary)] uppercase tracking-tight">
              Phoenix Recovery Console
            </h2>
          </div>
          {onClose && (
            <button
              onClick={() => {
                if (confirm('Close Recovery Console?')) {
                  onClose();
                }
              }}
              className="px-4 py-2 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded hover:bg-[var(--accent-hover)] transition-colors"
              disabled={restoring}
            >
              Close
            </button>
          )}
        </div>

        {/* Maintenance Mode Banner */}
        {maintenanceMode && (
          <div className="p-3 bg-[rgb(var(--warning-rgb)/0.1)] border border-[rgb(var(--warning-rgb)/0.3)] rounded mb-4">
            <div className="flex items-center gap-2">
              <span className="material-symbols-outlined text-[rgb(var(--warning-rgb)/0.9)] animate-pulse">
                build
              </span>
              <div>
                <div className="text-sm font-semibold text-[var(--text-primary)]">
                  Maintenance Mode Active
                </div>
                <div className="text-xs text-[var(--bg-steel)]">
                  PhoenixEvent bus traffic is paused. System operations are restricted.
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Search */}
        <div className="relative">
          <span className="material-symbols-outlined absolute left-3 top-1/2 transform -translate-y-1/2 text-[var(--bg-steel)]">
            search
          </span>
          <input
            type="text"
            placeholder="Search snapshots by ID or collection name..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2 bg-[var(--bg-muted)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded text-[var(--text-primary)] focus:outline-none focus:border-[var(--bg-steel)]"
          />
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {loading && (
          <div className="text-center py-12 text-[var(--bg-steel)]">
            <span className="material-symbols-outlined text-4xl mb-2 animate-spin">hourglass_empty</span>
            <p>Loading snapshots...</p>
          </div>
        )}

        {error && (
          <div className="p-4 bg-[rgb(var(--danger-rgb)/0.1)] border border-[rgb(var(--danger-rgb)/0.3)] text-[var(--text-primary)] rounded mb-4">
            <strong>Error:</strong> {error}
            <button
              onClick={fetchSnapshots}
              className="ml-4 px-3 py-1 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded text-sm hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] transition-colors"
            >
              Retry
            </button>
          </div>
        )}

        {!loading && !error && (
          <>
            {filteredSnapshots.length > 0 ? (
              <div className="space-y-4">
                <div className="text-sm text-[var(--bg-steel)] mb-4">
                  {filteredSnapshots.length} snapshot(s) found
                </div>
                {filteredSnapshots.map((snapshot, idx) => (
                  <div
                    key={`${snapshot.snapshot_id}-${snapshot.collection_name}-${idx}`}
                    className="bg-[rgb(var(--surface-rgb)/1)] rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.3)]"
                  >
                    <div className="flex items-start justify-between mb-4">
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-2">
                          <h3 className="text-lg font-bold text-[var(--text-primary)]">
                            {snapshot.snapshot_id}
                          </h3>
                          {snapshot.is_blessed && (
                            <span 
                              className="material-symbols-outlined text-[rgb(var(--warning-rgb)/1)]"
                              title="Blessed State: 95%+ compliance rating"
                            >
                              star
                            </span>
                          )}
                          {snapshot.is_recommended && !snapshot.is_blessed && (
                            <span 
                              className="text-xs px-2 py-0.5 bg-[rgb(var(--success-rgb)/0.2)] text-[rgb(var(--success-rgb)/1)] rounded border border-[rgb(var(--success-rgb)/0.3)]"
                              title="Recommended Recovery Point: Taken before compliance drift"
                            >
                              Recommended
                            </span>
                          )}
                        </div>
                        <div className="space-y-1 text-sm text-[var(--bg-steel)]">
                          <div className="flex items-center gap-2">
                            <span className="material-symbols-outlined text-sm">storage</span>
                            <span>Collection: <code className="text-[var(--text-primary)]">{snapshot.collection_name}</code></span>
                          </div>
                          <div className="flex items-center gap-2">
                            <span className="material-symbols-outlined text-sm">schedule</span>
                            <span>Created: {formatTimestamp(snapshot.creation_time)}</span>
                          </div>
                          <div className="flex items-center gap-2">
                            <span className="material-symbols-outlined text-sm">data_object</span>
                            <span>Size: {formatSize(snapshot.size)}</span>
                          </div>
                          {snapshot.compliance_score !== undefined && (
                            <div className="flex items-center gap-2">
                              <span className="material-symbols-outlined text-sm">verified</span>
                              <span>
                                Compliance: <span className={`font-semibold ${
                                  snapshot.compliance_score >= 95 ? 'text-[rgb(var(--success-rgb)/1)]' :
                                  snapshot.compliance_score >= 80 ? 'text-[rgb(var(--warning-rgb)/1)]' :
                                  'text-[rgb(var(--danger-rgb)/1)]'
                                }`}>
                                  {snapshot.compliance_score.toFixed(1)}%
                                </span>
                              </span>
                            </div>
                          )}
                          {snapshot.is_recommended && (
                            <div className="mt-2 p-2 bg-[rgb(var(--success-rgb)/0.1)] border border-[rgb(var(--success-rgb)/0.3)] rounded text-xs text-[var(--text-primary)]">
                              <span className="material-symbols-outlined text-sm align-middle mr-1">recommend</span>
                              Recommended Recovery Point: Snapshot taken before major compliance drift detected
                            </div>
                          )}
                        </div>
                      </div>
                      <button
                        onClick={() => handleRestore(snapshot)}
                        disabled={restoring === snapshot.snapshot_id || maintenanceMode}
                        className="px-4 py-2 bg-[rgb(var(--warning-rgb)/1)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--warning-rgb)/0.9)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm flex items-center gap-2"
                        title={maintenanceMode ? 'System is in maintenance mode' : 'Restore from this snapshot'}
                      >
                        <span className="material-symbols-outlined text-sm">
                          {restoring === snapshot.snapshot_id ? 'hourglass_empty' : 'restore'}
                        </span>
                        {restoring === snapshot.snapshot_id ? 'Restoring...' : 'Restore'}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-center py-12 text-[var(--bg-steel)]">
                <span className="material-symbols-outlined text-4xl mb-2">inbox</span>
                <p>
                  {searchQuery
                    ? 'No snapshots found matching your search.'
                    : 'No snapshots available. Create a snapshot first before attempting recovery.'}
                </p>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default PhoenixRecoveryConsole;
