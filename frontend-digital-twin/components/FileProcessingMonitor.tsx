import React, { useState, useEffect } from 'react';
import { getProcessingStats, FileProcessingStats } from '../services/projectService';
import { formatCompactBytes } from '../utils/formatMetrics';

interface FileProcessingMonitorProps {
  projectId?: string;
  onClose?: () => void;
}

const FileProcessingMonitor: React.FC<FileProcessingMonitorProps> = ({ projectId, onClose }) => {
  const [stats, setStats] = useState<FileProcessingStats[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);

  const loadStats = async () => {
    try {
      setError(null);
      const data = await getProcessingStats(projectId);
      setStats(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load processing stats');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadStats();
    if (autoRefresh) {
      const interval = setInterval(loadStats, 5000); // Refresh every 5 seconds
      return () => clearInterval(interval);
    }
  }, [projectId, autoRefresh]);

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'success': return 'text-[var(--bg-steel)] bg-[rgb(var(--bg-steel-rgb)/0.1)] border-[rgb(var(--bg-steel-rgb)/0.3)]';
      case 'error': return 'text-[var(--text-secondary)] bg-[rgb(var(--text-secondary-rgb)/0.1)] border-[rgb(var(--text-secondary-rgb)/0.3)]';
      case 'skipped': return 'text-[var(--bg-muted)] bg-[rgb(var(--bg-muted-rgb)/0.1)] border-[rgb(var(--bg-muted-rgb)/0.3)]';
      default: return 'text-[var(--text-secondary)] bg-[rgb(var(--surface-rgb)/0.3)] border-[rgb(var(--bg-steel-rgb)/0.25)]';
    }
  };

  const formatTimestamp = (timestamp: string): string => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  if (loading && stats.length === 0) {
    return (
      <div className="flex items-center justify-center p-8">
        <div className="text-[var(--text-secondary)]">Loading processing statistics...</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-[var(--bg-primary)]">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.3)]">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-bold text-[var(--text-secondary)] uppercase tracking-wider">
            File Processing Monitor
          </h2>
          {projectId && (
            <span className="text-xs text-[var(--bg-steel)] bg-[rgb(var(--surface-rgb)/0.5)] px-2 py-1 rounded">
              {stats[0]?.project_name || projectId}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-2 text-xs text-[var(--text-secondary)]">
            <input
              type="checkbox"
              checked={autoRefresh}
              onChange={(e) => setAutoRefresh(e.target.checked)}
              className="rounded"
            />
            Auto-refresh
          </label>
          <button
            onClick={loadStats}
            className="px-3 py-1 text-xs bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] text-[var(--text-on-accent)] rounded transition-colors"
          >
            Refresh
          </button>
          {onClose && (
            <button
              onClick={onClose}
              className="px-3 py-1 text-xs bg-[var(--text-secondary)] hover:bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded transition-colors"
            >
              Close
            </button>
          )}
        </div>
      </div>

      {error && (
        <div className="p-4 bg-[rgb(var(--surface-rgb)/0.5)] border border-[rgb(var(--text-secondary-rgb)/0.3)] m-4 rounded">
          <p className="text-sm text-[var(--text-secondary)]">{error}</p>
        </div>
      )}

      {/* Stats Cards */}
      <div className="p-4 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {stats.map((stat) => (
          <div key={stat.project_id} className="bg-[rgb(var(--surface-rgb)/0.5)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4">
            <h3 className="text-sm font-bold text-[var(--text-secondary)] uppercase tracking-wider mb-3">
              {stat.project_name}
            </h3>
            <div className="space-y-2">
              <div className="flex justify-between text-xs">
                <span className="text-[var(--bg-steel)]">Total Processed:</span>
                <span className="font-bold text-[var(--text-secondary)]">{stat.total_processed}</span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-[var(--bg-steel)]">Successful:</span>
                <span className="font-bold text-[var(--bg-steel)]">{stat.successful}</span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-[var(--bg-steel)]">Failed:</span>
                <span className="font-bold text-[var(--text-secondary)]">{stat.failed}</span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-[var(--bg-steel)]">Skipped:</span>
                <span className="font-bold text-[var(--bg-muted)]">{stat.skipped}</span>
              </div>
              {stat.last_processed && (
                <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                  <div className="text-[9px] text-[var(--bg-steel)]">
                    Last: {formatTimestamp(stat.last_processed)}
                  </div>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Recent Events */}
      <div className="flex-1 overflow-auto p-4">
        <h3 className="text-sm font-bold text-[var(--text-secondary)] uppercase tracking-wider mb-3">
          Recent Processing Events
        </h3>
        <div className="space-y-2">
          {stats.length === 0 ? (
            <div className="text-center py-8 text-[var(--bg-steel)]">
              No processing events yet. Files will appear here when detected.
            </div>
          ) : (
            stats.flatMap((stat) =>
              stat.recent_events.length === 0 ? (
                <div key={stat.project_id} className="text-center py-4 text-[var(--bg-steel)] text-xs">
                  No recent events for {stat.project_name}
                </div>
              ) : (
                stat.recent_events.map((event, idx) => (
                  <div
                    key={`${event.project_id}-${event.file_path}-${idx}`}
                    className="bg-[rgb(var(--surface-rgb)/0.5)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded p-3 hover:bg-[rgb(var(--surface-rgb)/0.7)] transition-colors"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1">
                          <span
                            className={`text-[9px] px-2 py-0.5 rounded border uppercase font-bold ${getStatusColor(
                              event.status
                            )}`}
                          >
                            {event.status}
                          </span>
                          <span className="text-xs font-bold text-[var(--text-secondary)] truncate">
                            {event.file_name}
                          </span>
                          <span className="text-[9px] text-[var(--bg-steel)]">
                            {formatCompactBytes(event.file_size)}
                          </span>
                        </div>
                        <div className="text-[10px] text-[var(--bg-steel)] truncate" title={event.file_path}>
                          {event.file_path}
                        </div>
                        {event.error_message && (
                          <div className="mt-1 text-[10px] text-[var(--text-secondary)]">
                            Error: {event.error_message}
                          </div>
                        )}
                        {event.memory_id && (
                          <div className="mt-1 text-[9px] text-[var(--bg-steel)]">
                            Memory ID: {event.memory_id.substring(0, 8)}...
                          </div>
                        )}
                      </div>
                      <div className="text-[9px] text-[var(--bg-steel)] shrink-0">
                        {formatTimestamp(event.timestamp)}
                      </div>
                    </div>
                  </div>
                ))
              )
            )
          )}
        </div>
      </div>
    </div>
  );
};

export default FileProcessingMonitor;
