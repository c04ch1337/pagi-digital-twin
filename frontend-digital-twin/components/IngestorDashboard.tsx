import React, { useEffect, useState, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useIngestStatus } from '../hooks/useIngestStatus';
import { useDomainAttribution } from '../context/DomainAttributionContext';
import HoverTooltip from './HoverTooltip';
import { Card, CardContent, CardHeader, CardTitle } from './ui/card';
import { Badge } from './ui/badge';
import toast from 'react-hot-toast';

interface FileProgress {
  fileName: string;
  domain: string;
  confidence: number;
  progress: number;
  status: 'processing' | 'complete' | 'failed';
  startTime: number;
}

interface IngestorDashboardProps {
  className?: string;
}

const IngestorDashboard: React.FC<IngestorDashboardProps> = ({ className = '' }) => {
  const { status } = useIngestStatus({ pollInterval: 2000 });
  const { incrementKnowledgeBase } = useDomainAttribution();
  const [activeFiles, setActiveFiles] = useState<Map<string, FileProgress>>(new Map());
  const previousStatusRef = useRef<typeof status | null>(null);
  const progressIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const [pendingToasts, setPendingToasts] = useState<Array<{ fileName: string; domain: string; confidence: number }>>([]);
  const toastThrottleRef = useRef<NodeJS.Timeout | null>(null);

  // Infer domain from file name or content
  const inferDomain = (fileName: string): { domain: string; confidence: number } => {
    const lower = fileName.toLowerCase();
    
    // Soul (Ethical/Governance)
    if (lower.includes('security') || lower.includes('audit') || lower.includes('compliance') || 
        lower.includes('policy') || lower.includes('governance') || lower.includes('soul')) {
      return { domain: 'Soul', confidence: 0.95 };
    }
    
    // Body (System/Telemetry)
    if (lower.includes('log') || lower.includes('telemetry') || lower.includes('metric') || 
        lower.includes('system') || lower.includes('performance') || lower.includes('body')) {
      return { domain: 'Body', confidence: 0.92 };
    }
    
    // Heart (Personal/User)
    if (lower.includes('user') || lower.includes('persona') || lower.includes('preference') || 
        lower.includes('feedback') || lower.includes('heart')) {
      return { domain: 'Heart', confidence: 0.90 };
    }
    
    // Mind (Technical/Intellectual) - default
    if (lower.includes('spec') || lower.includes('api') || lower.includes('config') || 
        lower.includes('manual') || lower.includes('guide') || lower.includes('mind') ||
        lower.includes('tech') || lower.includes('code')) {
      return { domain: 'Mind', confidence: 0.93 };
    }
    
    // Default to Mind if unclear
    return { domain: 'Mind', confidence: 0.75 };
  };

  // Get domain color
  const getDomainColor = (domain: string): string => {
    switch (domain) {
      case 'Mind':
        return 'bg-[var(--bg-steel)]'; // Blue
      case 'Body':
        return 'bg-[rgb(var(--success-rgb))]'; // Green
      case 'Heart':
        return 'bg-[rgb(var(--warning-rgb))]'; // Orange
      case 'Soul':
        return 'bg-[rgb(var(--danger-rgb))]'; // Red
      default:
        return 'bg-[var(--bg-steel)]';
    }
  };

  // Get domain border color
  const getDomainBorderColor = (domain: string): string => {
    switch (domain) {
      case 'Mind':
        return 'border-[var(--bg-steel)]';
      case 'Body':
        return 'border-[rgb(var(--success-rgb))]';
      case 'Heart':
        return 'border-[rgb(var(--warning-rgb))]';
      case 'Soul':
        return 'border-[rgb(var(--danger-rgb))]';
      default:
        return 'border-[var(--bg-steel)]';
    }
  };

  // Get domain icon
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

  // Detect file state changes and update progress
  useEffect(() => {
    if (!previousStatusRef.current) {
      previousStatusRef.current = status;
      return;
    }

    const prev = previousStatusRef.current;
    const current = status;

    // Detect new file starting
    if (current.is_active && current.current_file && 
        (!prev.is_active || prev.current_file !== current.current_file)) {
      const fileName = current.current_file.split('/').pop() || 'Unknown';
      const { domain, confidence } = inferDomain(fileName);
      
      setActiveFiles(prev => {
        const newMap = new Map(prev);
        newMap.set(fileName, {
          fileName,
          domain,
          confidence,
          progress: 0,
          status: 'processing',
          startTime: Date.now(),
        });
        return newMap;
      });
    }

    // Detect file completion
    if (prev.is_active && prev.current_file && 
        !current.is_active && !current.current_file) {
      const fileName = prev.current_file.split('/').pop() || 'Unknown';
      const previousStatus = prev; // Store previous status for comparison
      
      setActiveFiles(prevFiles => {
        const newMap = new Map(prevFiles);
        const file = newMap.get(fileName);
        if (file) {
          // Check if it was successful
          if (current.files_processed > previousStatus.files_processed) {
            newMap.set(fileName, {
              ...file,
              progress: 100,
              status: 'complete',
            });
            
            // Add to pending toasts queue (throttled to avoid UI spam)
            setPendingToasts(prev => [...prev, { fileName, domain: file.domain, confidence: file.confidence }]);
            
            // Throttle toast notifications - batch if more than 5 files complete at once
            if (toastThrottleRef.current) {
              clearTimeout(toastThrottleRef.current);
            }
            
            toastThrottleRef.current = setTimeout(() => {
              setPendingToasts(prev => {
                if (prev.length === 0) return prev;
                
                if (prev.length === 1) {
                  // Single file - show individual toast
                  const item = prev[0];
                  toast.success(
                    `${item.fileName} classified as ${item.domain} (${Math.round(item.confidence * 100)}% confidence)`,
                    {
                      duration: 4000,
                      icon: '📚',
                    }
                  );
                } else if (prev.length <= 5) {
                  // Small batch - show individual toasts
                  prev.forEach(item => {
                    toast.success(
                      `${item.fileName} → ${item.domain} (${Math.round(item.confidence * 100)}%)`,
                      {
                        duration: 3000,
                        icon: '📚',
                      }
                    );
                  });
                } else {
                  // Large batch - show summary toast
                  const domainCounts = prev.reduce((acc, item) => {
                    acc[item.domain] = (acc[item.domain] || 0) + 1;
                    return acc;
                  }, {} as Record<string, number>);
                  
                  const summary = Object.entries(domainCounts)
                    .map(([domain, count]) => `${domain}: ${count}`)
                    .join(', ');
                  
                  toast.success(
                    `${prev.length} files ingested: ${summary}`,
                    {
                      duration: 5000,
                      icon: '📚',
                    }
                  );
                }
                
                return [];
              });
            }, 500); // Batch toasts within 500ms window

            // Increment knowledge base stats
            incrementKnowledgeBase(file.domain as 'Mind' | 'Body' | 'Heart' | 'Soul');

            // Dispatch custom event for AttributionAnalytics
            window.dispatchEvent(new CustomEvent('ingestion-complete', {
              detail: { domain: file.domain, fileName }
            }));

            // Remove from active files after a delay
            setTimeout(() => {
              setActiveFiles(prev => {
                const newMap = new Map(prev);
                newMap.delete(fileName);
                return newMap;
              });
            }, 3000);
          } else if (current.files_failed > previousStatus.files_failed) {
            newMap.set(fileName, {
              ...file,
              status: 'failed',
            });
            
            toast.error(`Failed to ingest ${fileName}`, {
              duration: 4000,
            });

            setTimeout(() => {
              setActiveFiles(prev => {
                const newMap = new Map(prev);
                newMap.delete(fileName);
                return newMap;
              });
            }, 3000);
          }
        }
        return newMap;
      });
    }

    previousStatusRef.current = current;
  }, [status, incrementKnowledgeBase]);

  // Simulate progress for active files
  useEffect(() => {
    progressIntervalRef.current = setInterval(() => {
      setActiveFiles(prev => {
        const newMap = new Map(prev);
        newMap.forEach((file, key) => {
          if (file.status === 'processing') {
            // Simulate progress based on time elapsed (0-90% over ~10 seconds)
            const elapsed = Date.now() - file.startTime;
            const progress = Math.min(90, (elapsed / 10000) * 90);
            newMap.set(key, { ...file, progress });
          }
        });
        return newMap;
      });
    }, 500);

    return () => {
      if (progressIntervalRef.current) {
        clearInterval(progressIntervalRef.current);
      }
    };
  }, []);

  // Cleanup toast throttle on unmount
  useEffect(() => {
    return () => {
      if (toastThrottleRef.current) {
        clearTimeout(toastThrottleRef.current);
      }
    };
  }, []);

  const activeFilesArray = Array.from(activeFiles.values());

  return (
    <Card className={className}>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <HoverTooltip
            title="Ingestion Progress Dashboard"
            description="Real-time visualization of knowledge ingestion. Files are automatically classified into Mind/Body/Heart/Soul domains and ingested into Qdrant vector storage."
          >
            <div className="flex items-center gap-2 cursor-help">
              <span className="material-symbols-outlined text-[14px] text-[var(--bg-steel)]">
                auto_awesome
              </span>
              <CardTitle className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                Ingestion Progress
              </CardTitle>
            </div>
          </HoverTooltip>
          {status.is_active && (
            <div className="flex items-center gap-1 text-[9px] text-[var(--text-secondary)]">
              <div className="w-2 h-2 rounded-full bg-[rgb(var(--success-rgb))] animate-pulse" />
              <span className="font-bold uppercase">Active</span>
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent className="pt-0">

      {/* Summary Stats */}
      <div className="grid grid-cols-3 gap-2 mb-4">
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
            Active
          </div>
          <div className="text-lg font-black text-[var(--text-primary)]">
            {activeFilesArray.filter(f => f.status === 'processing').length}
          </div>
        </div>
      </div>

      {/* Active Files List */}
      <div className="space-y-2">
        <AnimatePresence>
          {activeFilesArray.length === 0 ? (
            <div className="text-center py-4">
              <div className="text-[9px] text-[var(--text-secondary)] opacity-70 italic">
                {status.is_active 
                  ? 'Processing files...' 
                  : 'No active ingestion. Drop files into data/ingest/ to begin.'}
              </div>
            </div>
          ) : (
            activeFilesArray.map((file) => (
              <motion.div
                key={file.fileName}
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.95 }}
                transition={{ duration: 0.2 }}
                className={`bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg p-3 border ${getDomainBorderColor(file.domain)} border-opacity-30`}
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2 min-w-0 flex-1">
                    <span className={`material-symbols-outlined text-[12px] ${getDomainColor(file.domain)}`}>
                      {getDomainIcon(file.domain)}
                    </span>
                    <span className="text-[10px] font-bold text-[var(--text-primary)] truncate">
                      {file.fileName}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge 
                      variant={file.domain === 'Soul' ? 'destructive' : file.domain === 'Heart' ? 'secondary' : 'default'}
                      className="text-[9px]"
                    >
                      {file.domain}
                    </Badge>
                    {file.status === 'processing' && (
                      <span className="text-[8px] text-[var(--text-secondary)]">
                        {Math.round(file.progress)}%
                      </span>
                    )}
                    {file.status === 'complete' && (
                      <span className="text-[8px] text-[rgb(var(--success-rgb))] font-bold">
                        ✓
                      </span>
                    )}
                    {file.status === 'failed' && (
                      <span className="text-[8px] text-[rgb(var(--danger-rgb))] font-bold">
                        ✗
                      </span>
                    )}
                  </div>
                </div>
                
                {/* Progress Bar */}
                {file.status === 'processing' && (
                  <div className="w-full h-1.5 bg-[rgb(var(--bg-steel-rgb)/0.2)] rounded-full overflow-hidden">
                    <motion.div
                      className={`h-full ${getDomainColor(file.domain)}`}
                      initial={{ width: 0 }}
                      animate={{ width: `${file.progress}%` }}
                      transition={{ duration: 0.5, ease: 'easeOut' }}
                    />
                  </div>
                )}
                
                {file.status === 'complete' && (
                  <div className="w-full h-1.5 bg-[rgb(var(--success-rgb)/0.3)] rounded-full">
                    <div className="h-full bg-[rgb(var(--success-rgb))] w-full" />
                  </div>
                )}
                
                {file.status === 'failed' && (
                  <div className="w-full h-1.5 bg-[rgb(var(--danger-rgb)/0.3)] rounded-full">
                    <div className="h-full bg-[rgb(var(--danger-rgb))] w-full" />
                  </div>
                )}

                {/* Confidence Badge */}
                {file.status === 'processing' && (
                  <div className="mt-1.5 text-[8px] text-[var(--text-secondary)] opacity-70">
                    Confidence: {Math.round(file.confidence * 100)}%
                  </div>
                )}
              </motion.div>
            ))
          )}
        </AnimatePresence>
      </div>

      {/* Last Error */}
      {status.last_error && (
        <div className="mt-3 p-2 bg-[rgb(var(--danger-rgb)/0.1)] border border-[rgb(var(--danger-rgb)/0.3)] rounded-lg">
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
      </CardContent>
    </Card>
  );
};

export default IngestorDashboard;
