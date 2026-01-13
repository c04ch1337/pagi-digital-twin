import React, { useState, useEffect } from 'react';
import { getProcessingStats, FileProcessingStats } from '../services/projectService';

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
      case 'success': return 'text-[#5381A5] bg-[#5381A5]/10 border-[#5381A5]/30';
      case 'error': return 'text-[#163247] bg-[#163247]/10 border-[#163247]/30';
      case 'skipped': return 'text-[#78A2C2] bg-[#78A2C2]/10 border-[#78A2C2]/30';
      default: return 'text-[#163247] bg-white/30 border-[#5381A5]/25';
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
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
        <div className="text-[#163247]">Loading processing statistics...</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-[#9EC9D9]">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-[#5381A5]/30 bg-white/30">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-bold text-[#163247] uppercase tracking-wider">
            File Processing Monitor
          </h2>
          {projectId && (
            <span className="text-xs text-[#5381A5] bg-white/50 px-2 py-1 rounded">
              {stats[0]?.project_name || projectId}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-2 text-xs text-[#163247]">
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
            className="px-3 py-1 text-xs bg-[#5381A5] hover:bg-[#78A2C2] text-white rounded transition-colors"
          >
            Refresh
          </button>
          {onClose && (
            <button
              onClick={onClose}
              className="px-3 py-1 text-xs bg-[#163247] hover:bg-[#5381A5] text-white rounded transition-colors"
            >
              Close
            </button>
          )}
        </div>
      </div>

      {error && (
        <div className="p-4 bg-white/50 border border-[#163247]/30 m-4 rounded">
          <p className="text-sm text-[#163247]">{error}</p>
        </div>
      )}

      {/* Stats Cards */}
      <div className="p-4 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {stats.map((stat) => (
          <div key={stat.project_id} className="bg-white/50 border border-[#5381A5]/30 rounded-lg p-4">
            <h3 className="text-sm font-bold text-[#163247] uppercase tracking-wider mb-3">
              {stat.project_name}
            </h3>
            <div className="space-y-2">
              <div className="flex justify-between text-xs">
                <span className="text-[#5381A5]">Total Processed:</span>
                <span className="font-bold text-[#163247]">{stat.total_processed}</span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-[#5381A5]">Successful:</span>
                <span className="font-bold text-[#5381A5]">{stat.successful}</span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-[#5381A5]">Failed:</span>
                <span className="font-bold text-[#163247]">{stat.failed}</span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-[#5381A5]">Skipped:</span>
                <span className="font-bold text-[#78A2C2]">{stat.skipped}</span>
              </div>
              {stat.last_processed && (
                <div className="mt-3 pt-3 border-t border-[#5381A5]/20">
                  <div className="text-[9px] text-[#5381A5]">
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
        <h3 className="text-sm font-bold text-[#163247] uppercase tracking-wider mb-3">
          Recent Processing Events
        </h3>
        <div className="space-y-2">
          {stats.length === 0 ? (
            <div className="text-center py-8 text-[#5381A5]">
              No processing events yet. Files will appear here when detected.
            </div>
          ) : (
            stats.flatMap((stat) =>
              stat.recent_events.length === 0 ? (
                <div key={stat.project_id} className="text-center py-4 text-[#5381A5] text-xs">
                  No recent events for {stat.project_name}
                </div>
              ) : (
                stat.recent_events.map((event, idx) => (
                  <div
                    key={`${event.project_id}-${event.file_path}-${idx}`}
                    className="bg-white/50 border border-[#5381A5]/30 rounded p-3 hover:bg-white/70 transition-colors"
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
                          <span className="text-xs font-bold text-[#163247] truncate">
                            {event.file_name}
                          </span>
                          <span className="text-[9px] text-[#5381A5]">
                            {formatFileSize(event.file_size)}
                          </span>
                        </div>
                        <div className="text-[10px] text-[#5381A5] truncate" title={event.file_path}>
                          {event.file_path}
                        </div>
                        {event.error_message && (
                          <div className="mt-1 text-[10px] text-[#163247]">
                            Error: {event.error_message}
                          </div>
                        )}
                        {event.memory_id && (
                          <div className="mt-1 text-[9px] text-[#5381A5]">
                            Memory ID: {event.memory_id.substring(0, 8)}...
                          </div>
                        )}
                      </div>
                      <div className="text-[9px] text-[#5381A5] shrink-0">
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
