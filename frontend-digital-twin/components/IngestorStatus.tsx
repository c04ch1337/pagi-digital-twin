import React, { useEffect, useState } from 'react';
import { getIngestStatus, IngestionStatus } from '../services/knowledgeService';
import HoverTooltip from './HoverTooltip';

interface IngestorStatusProps {
  className?: string;
  onIngestionComplete?: (domain: string, fileName: string) => void;
}

const IngestorStatus: React.FC<IngestorStatusProps> = ({ 
  className = '',
  onIngestionComplete 
}) => {
  const [status, setStatus] = useState<IngestionStatus>({
    is_active: false,
    files_processed: 0,
    files_failed: 0,
    current_file: null,
    last_error: null,
  });
  const [isPolling, setIsPolling] = useState(true);
  const [previousStatus, setPreviousStatus] = useState<IngestionStatus | null>(null);

  // Poll for status updates
  useEffect(() => {
    if (!isPolling) return;

    const pollInterval = setInterval(async () => {
      try {
        const newStatus = await getIngestStatus();
        setStatus(newStatus);

        // Detect when a file finishes processing
        if (previousStatus) {
          // If we were processing a file and now we're not, a file completed
          if (previousStatus.is_active && previousStatus.current_file && 
              !newStatus.is_active && !newStatus.current_file) {
            // File completed successfully
            if (newStatus.files_processed > previousStatus.files_processed) {
              const fileName = previousStatus.current_file.split('/').pop() || 'Unknown';
              // Try to infer domain from file name or use a default
              const domain = inferDomainFromFileName(fileName);
              onIngestionComplete?.(domain, fileName);
            }
          }
        }

        setPreviousStatus(newStatus);
      } catch (error) {
        console.error('[IngestorStatus] Error polling status:', error);
      }
    }, 2000); // Poll every 2 seconds

    return () => clearInterval(pollInterval);
  }, [isPolling, previousStatus, onIngestionComplete]);

  // Infer domain from file name (simple heuristic)
  const inferDomainFromFileName = (fileName: string): string => {
    const lower = fileName.toLowerCase();
    if (lower.includes('security') || lower.includes('audit') || lower.includes('compliance') || 
        lower.includes('policy') || lower.includes('governance')) {
      return 'Soul';
    }
    if (lower.includes('log') || lower.includes('telemetry') || lower.includes('metric') || 
        lower.includes('system') || lower.includes('performance')) {
      return 'Body';
    }
    if (lower.includes('user') || lower.includes('persona') || lower.includes('preference') || 
        lower.includes('feedback')) {
      return 'Heart';
    }
    if (lower.includes('spec') || lower.includes('api') || lower.includes('config') || 
        lower.includes('manual') || lower.includes('guide')) {
      return 'Mind';
    }
    return 'Unknown';
  };

  const getDomainColor = (domain: string): string => {
    switch (domain) {
      case 'Mind':
        return 'bg-[var(--bg-steel)]';
      case 'Body':
        return 'bg-[var(--bg-steel)]';
      case 'Heart':
        return 'bg-[rgb(var(--warning-rgb))]';
      case 'Soul':
        return 'bg-[rgb(var(--danger-rgb))]';
      default:
        return 'bg-[var(--bg-steel)]';
    }
  };

  const getDomainIcon = (domain: string): string => {
    switch (domain) {
      case 'Mind':
        return 'psychology';
      case 'Body':
        return 'monitor';
      case 'Heart':
        return 'favorite';
      case 'Soul':
        return 'shield';
      default:
        return 'description';
    }
  };

  const totalFiles = status.files_processed + status.files_failed;
  const successRate = totalFiles > 0 
    ? Math.round((status.files_processed / totalFiles) * 100) 
    : 100;

  return (
    <div className={`bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 ${className}`}>
      <div className="flex items-center justify-between mb-3">
        <HoverTooltip
          title="Auto-Domain Ingestor"
          description="Monitors the knowledge ingestion pipeline. Files dropped into data/ingest/ are automatically classified and routed to the correct domain (Mind/Body/Heart/Soul)."
        >
          <div className="flex items-center gap-2 cursor-help">
            <span className="material-symbols-outlined text-[14px] text-[var(--bg-steel)]">
              auto_awesome
            </span>
            <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
              Knowledge Ingestor
            </h3>
          </div>
        </HoverTooltip>
        <div className="flex items-center gap-2">
          {status.is_active && (
            <div className="flex items-center gap-1 text-[9px] text-[var(--text-secondary)]">
              <div className="w-2 h-2 rounded-full bg-[rgb(var(--success-rgb))] animate-pulse" />
              <span className="font-bold uppercase">Active</span>
            </div>
          )}
        </div>
      </div>

      {/* Status Summary */}
      <div className="grid grid-cols-3 gap-2 mb-3">
        <div className="bg-[rgb(var(--bg-steel-rgb)/0.1)] rounded-lg p-2">
          <div className="text-[9px] text-[var(--text-secondary)] uppercase font-bold mb-1">
            Processed
          </div>
          <div className="text-lg font-black text-[var(--text-primary)]">
            {status.files_processed}
          </div>
        </div>
        <div className="bg-[rgb(var(--bg-steel-rgb)/0.1)] rounded-lg p-2">
          <div className="text-[9px] text-[var(--text-secondary)] uppercase font-bold mb-1">
            Failed
          </div>
          <div className="text-lg font-black text-[rgb(var(--danger-rgb))]">
            {status.files_failed}
          </div>
        </div>
        <div className="bg-[rgb(var(--bg-steel-rgb)/0.1)] rounded-lg p-2">
          <div className="text-[9px] text-[var(--text-secondary)] uppercase font-bold mb-1">
            Success Rate
          </div>
          <div className="text-lg font-black text-[var(--text-primary)]">
            {successRate}%
          </div>
        </div>
      </div>

      {/* Current File Progress */}
      {status.is_active && status.current_file && (
        <div className="mb-3">
          <div className="flex items-center justify-between mb-1">
            <div className="flex items-center gap-2">
              <span className="material-symbols-outlined text-[12px] text-[var(--text-secondary)]">
                description
              </span>
              <span className="text-[10px] text-[var(--text-secondary)] font-bold truncate max-w-[200px]">
                {status.current_file.split('/').pop()}
              </span>
            </div>
            <span className="text-[9px] text-[var(--text-secondary)] uppercase">
              Processing...
            </span>
          </div>
          <div className="w-full h-1.5 bg-[rgb(var(--bg-steel-rgb)/0.2)] rounded-full overflow-hidden">
            <div 
              className="h-full bg-[rgb(var(--success-rgb))] rounded-full transition-all duration-300 animate-pulse"
              style={{ width: '60%' }} // Simulated progress
            />
          </div>
        </div>
      )}

      {/* Last Error */}
      {status.last_error && (
        <div className="mb-3 p-2 bg-[rgb(var(--danger-rgb)/0.1)] border border-[rgb(var(--danger-rgb)/0.3)] rounded-lg">
          <div className="flex items-start gap-2">
            <span className="material-symbols-outlined text-[12px] text-[rgb(var(--danger-rgb))]">
              error
            </span>
            <div className="flex-1">
              <div className="text-[9px] text-[rgb(var(--danger-rgb))] uppercase font-bold mb-1">
                Last Error
              </div>
              <div className="text-[10px] text-[var(--text-secondary)]">
                {status.last_error}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Idle State */}
      {!status.is_active && !status.current_file && totalFiles === 0 && (
        <div className="text-center py-4">
          <div className="text-[9px] text-[var(--text-secondary)] opacity-70 italic">
            Waiting for files in <code className="text-[8px]">data/ingest/</code>
          </div>
        </div>
      )}

      {/* Recent Activity Summary */}
      {totalFiles > 0 && !status.is_active && (
        <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
          <div className="text-[9px] text-[var(--text-secondary)] uppercase font-bold mb-2">
            Recent Activity
          </div>
          <div className="text-[10px] text-[var(--text-secondary)] opacity-80">
            {status.files_processed} file{status.files_processed !== 1 ? 's' : ''} successfully ingested
            {status.files_failed > 0 && (
              <span className="text-[rgb(var(--danger-rgb))]">
                {' '}• {status.files_failed} failed
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default IngestorStatus;
