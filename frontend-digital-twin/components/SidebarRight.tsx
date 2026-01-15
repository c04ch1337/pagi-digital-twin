import React, { useState, useEffect } from 'react';
import { Job, Approval, Twin } from '../types';
import { ICONS } from '../constants';
import TelemetryCharts from './TelemetryCharts';
import NeuralMemorySearch from './NeuralMemorySearch';
import MemoryHealth from './MemoryHealth';
import HoverTooltip from './HoverTooltip';
import IngestorDashboard from './IngestorDashboard';
import { fetchNamespaceMetrics, MemoryStatus } from '../services/memory';
import { useTelemetry } from '../context/TelemetryContext';
import { fetchSyncMetrics } from '../services/systemService';

import { DomainAttribution } from '../types/protocol';

interface SidebarRightProps {
  jobs: Job[];
  approvals: Approval[];
  onApprove: (id: string) => void;
  onDeny: (id: string) => void;
  activeTwin: Twin;
  onViewLogs: (jobId: string) => void;
  domainAttribution?: DomainAttribution;
}

const SidebarRight: React.FC<SidebarRightProps> = ({ jobs, approvals, onApprove, onDeny, activeTwin, onViewLogs, domainAttribution }) => {
  // Get telemetry data from context (SSE stream)
  const { telemetry, isConnected: isTelemetryConnected } = useTelemetry();
  const [memoryInfo, setMemoryInfo] = useState<MemoryStatus | null>(null);
  const [neuralSync, setNeuralSync] = useState<number>(100);
  const [showAttributionDetails, setShowAttributionDetails] = useState<boolean>(false);

  const latest = telemetry.length > 0 ? telemetry[telemetry.length - 1] : { cpu: 0, memory: 0, network: 0, gpu: 0, timestamp: '' };

  // --- Threshold-based UI feedback for CPU/RAM ---
  // Values are percents (0..100). Tune as desired.
  const METRIC_THRESHOLDS = {
    cpu: { high: 70, critical: 90 },
    memory: { high: 75, critical: 90 },
  } as const;

  type MetricKey = keyof typeof METRIC_THRESHOLDS;
  type MetricSeverity = 'normal' | 'high' | 'critical';

  const metricSeverity = (metric: MetricKey, value: number): MetricSeverity => {
    const v = Number.isFinite(value) ? value : 0;
    const t = METRIC_THRESHOLDS[metric];
    if (v >= t.critical) return 'critical';
    if (v >= t.high) return 'high';
    return 'normal';
  };

  const metricCardClasses = (severity: MetricSeverity): string => {
    // Keep the base style consistent with the existing UI, but add color + pulse when elevated.
    // When telemetry is offline, we intentionally disable pulsing to avoid “false alarm” visuals.
    const offline = !isTelemetryConnected;
    const base = 'bg-[rgb(var(--surface-rgb)/0.3)] p-2 rounded-lg border transition-colors';
    const offlineMute = offline ? ' opacity-60' : '';

    switch (severity) {
      case 'critical':
        return `${base} border-[rgb(var(--danger-rgb)/0.6)] bg-[rgb(var(--danger-rgb)/0.12)] ${offline ? '' : 'animate-pulse'} ring-1 ring-[rgb(var(--danger-rgb)/0.2)]${offlineMute}`;
      case 'high':
        return `${base} border-[rgb(var(--warning-rgb)/0.5)] bg-[rgb(var(--warning-rgb)/0.12)] ${offline ? '' : 'animate-pulse'}${offlineMute}`;
      case 'normal':
      default:
        return `${base} border-[rgb(var(--bg-steel-rgb)/0.3)]${offlineMute}`;
    }
  };

  const metricValueClasses = (severity: MetricSeverity): string => {
    switch (severity) {
      case 'critical':
        return 'text-[rgb(var(--danger-rgb)/0.85)]';
      case 'high':
        return 'text-[rgb(var(--warning-rgb)/0.95)]';
      case 'normal':
      default:
        return 'text-[var(--text-primary)]';
    }
  };

  const metricLabel = (severity: MetricSeverity): string => {
    switch (severity) {
      case 'critical':
        return 'CRITICAL';
      case 'high':
        return 'HIGH';
      default:
        return '';
    }
  };

  useEffect(() => {
    const updateMemory = () => {
      const metrics = fetchNamespaceMetrics(activeTwin.settings.memoryNamespace);
      setMemoryInfo(metrics);
    };
    
    updateMemory();
    const interval = setInterval(updateMemory, 2000);
    return () => clearInterval(interval);
  }, [activeTwin.settings.memoryNamespace]);

  // Fetch Neural Sync metrics periodically
  useEffect(() => {
    const updateSyncMetrics = async () => {
      try {
        const metrics = await fetchSyncMetrics();
        setNeuralSync(Math.round(metrics.neural_sync));
      } catch (error) {
        console.error('[SidebarRight] Failed to fetch sync metrics:', error);
        // On error, keep the last known value or default to 100
      }
    };

    updateSyncMetrics();
    const interval = setInterval(updateSyncMetrics, 5000); // Update every 5 seconds
    return () => clearInterval(interval);
  }, []);


  const getStatusColor = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'text-[var(--bg-steel)]';
      case 'failed': return 'text-[var(--text-secondary)]';
      case 'active': return 'text-[var(--bg-steel)]';
      case 'pending': return 'text-[var(--bg-muted)]';
      default: return 'text-[var(--text-secondary)]';
    }
  };

  const getStatusBg = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'bg-[var(--bg-steel)]';
      case 'failed': return 'bg-[var(--text-secondary)]';
      case 'active': return 'bg-[var(--bg-steel)]';
      case 'pending': return 'bg-[var(--bg-muted)]';
      default: return 'bg-[var(--text-secondary)]';
    }
  };

  return (
    <aside className="w-80 bg-[var(--bg-secondary)] border-l border-[rgb(var(--bg-steel-rgb)/0.3)] flex flex-col shrink-0 relative">
      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {/* BODY - SYSTEM TELEMETRY */}
        <section className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)]">
          <div className="flex items-center justify-between mb-4">
             <HoverTooltip
               title="System Telemetry"
               description="Live hardware telemetry (CPU, memory, and network) streamed from the Telemetry service and displayed as recent time-series samples."
             >
               <div className="flex items-center gap-2">
                 <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--bg-steel)] rounded">
                   <ICONS.Activity />
                 </div>
                 <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">Body (System)</h3>
               </div>
             </HoverTooltip>

             <HoverTooltip
               title="Telemetry Link"
               description="Connection status to the telemetry stream (SSE). LIVE means the UI is receiving fresh samples; OFFLINE means telemetry updates are not arriving."
             >
               <div className="flex items-center gap-2">
                 <span className={`w-1.5 h-1.5 rounded-full ${isTelemetryConnected ? 'bg-[var(--bg-steel)] animate-pulse' : 'bg-[var(--bg-muted)]'}`}></span>
                 <span className="text-[10px] text-[var(--text-secondary)] mono">
                   {isTelemetryConnected ? 'LIVE' : 'OFFLINE'}
                 </span>
               </div>
             </HoverTooltip>
           </div>
           <div className="space-y-4">
             <HoverTooltip
               title="Telemetry Charts"
               description="Displays the last ~30 samples. Hover a line to see the point-in-time value. Values are percentages (0–100)."
             >
               <div className="bg-[rgb(var(--surface-rgb)/0.4)] rounded-xl p-3 border border-[rgb(var(--bg-steel-rgb)/0.3)]">
                 <TelemetryCharts data={telemetry} />
               </div>
             </HoverTooltip>
               
               <div className="grid grid-cols-2 gap-2">
                 <HoverTooltip
                   title="CPU Load"
                   description="Current CPU utilization (percent). This is the latest sample and may fluctuate rapidly during builds/executions."
                 >
                  {(() => {
                    const sev = metricSeverity('cpu', Number(latest.cpu));
                    return (
                      <div className={metricCardClasses(sev)}>
                        <div className="flex items-center justify-between gap-2 mb-1">
                          <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-tighter">CPU LOAD</div>
                          {metricLabel(sev) && (
                            <div
                               className={`text-[8px] font-black uppercase tracking-widest px-1.5 py-0.5 rounded border ${
                                 sev === 'critical'
                                   ? 'text-[rgb(var(--danger-rgb)/0.85)] border-[rgb(var(--danger-rgb)/0.4)] bg-[rgb(var(--danger-rgb)/0.12)]'
                                   : 'text-[rgb(var(--warning-rgb)/0.95)] border-[rgb(var(--warning-rgb)/0.4)] bg-[rgb(var(--warning-rgb)/0.12)]'
                               }`}
                               title={`Threshold: ${sev}`}
                             >
                              {metricLabel(sev)}
                            </div>
                          )}
                        </div>
                        <div className={`text-sm font-bold mono ${metricValueClasses(sev)}`}>{Number(latest.cpu).toFixed(2)}%</div>
                      </div>
                    );
                  })()}
                  </HoverTooltip>

                 <HoverTooltip
                   title="RAM Core"
                   description="Current memory utilization (percent). High values can indicate memory pressure and may impact tool execution performance."
                 >
                  {(() => {
                    const sev = metricSeverity('memory', Number(latest.memory));
                    return (
                      <div className={metricCardClasses(sev)}>
                        <div className="flex items-center justify-between gap-2 mb-1">
                          <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-tighter">RAM CORE</div>
                          {metricLabel(sev) && (
                            <div
                               className={`text-[8px] font-black uppercase tracking-widest px-1.5 py-0.5 rounded border ${
                                 sev === 'critical'
                                   ? 'text-[rgb(var(--danger-rgb)/0.85)] border-[rgb(var(--danger-rgb)/0.4)] bg-[rgb(var(--danger-rgb)/0.12)]'
                                   : 'text-[rgb(var(--warning-rgb)/0.95)] border-[rgb(var(--warning-rgb)/0.4)] bg-[rgb(var(--warning-rgb)/0.12)]'
                               }`}
                               title={`Threshold: ${sev}`}
                             >
                              {metricLabel(sev)}
                            </div>
                          )}
                        </div>
                        <div className={`text-sm font-bold mono ${metricValueClasses(sev)}`}>{Number(latest.memory).toFixed(2)}%</div>
                      </div>
                    );
                  })()}
                  </HoverTooltip>
               </div>
            </div>
          </section>

        {/* MIND - VECTOR VAULT & SEMANTIC SEARCH */}
        <section className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)]">
          <HoverTooltip
            title="Mind (Vector Vault)"
            description="Semantic memory storage + query surface for the active twin. Shows the current namespace health and provides a vector search UI."
          >
            <div className="flex items-center gap-2 mb-4 cursor-help">
              <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--bg-steel)] rounded">
                <ICONS.Brain />
              </div>
              <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">Mind (Vector Vault)</h3>
            </div>
          </HoverTooltip>

           <div className="space-y-4">
              {/* Namespace Status */}
              <HoverTooltip
                title="Namespace Status"
                description="Active memory namespace for this twin (Vector Vault). Shards indicate storage partitions; the bar approximates current load/pressure for the namespace."
              >
                <div className="p-3 bg-[rgb(var(--surface-rgb)/0.3)] rounded-xl border border-[rgb(var(--bg-steel-rgb)/0.3)] cursor-help">
                  <div className="flex justify-between items-center mb-2">
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-[12px] text-[var(--bg-steel)]">database</span>
                      <div className="text-[10px] font-bold text-[var(--text-primary)] uppercase tracking-widest">{activeTwin.settings.memoryNamespace}</div>
                    </div>
                    <span className="text-[9px] font-mono text-[var(--bg-steel)] font-bold">{memoryInfo?.shardCount || 0} Shards</span>
                  </div>
                  <div className="flex gap-1 h-1.5 mb-1">
                    {Array.from({ length: 12 }).map((_, i) => (
                      <div
                        key={i}
                        className={`flex-1 rounded-sm transition-all duration-300 ${
                          i < Math.ceil((memoryInfo?.load || 0) / 8.33) ? 'bg-[var(--bg-steel)]' : 'bg-[rgb(var(--surface-rgb)/0.4)]'
                        }`}
                      />
                    ))}
                  </div>
                </div>
              </HoverTooltip>

              {/* Neural Memory Search */}
              <HoverTooltip
                title="Neural Memory Search"
                description="Run a semantic (vector) query against the active namespace. Use keywords (e.g., agent id, risk level) to locate stored context quickly."
              >
                <div>
                  <NeuralMemorySearch activeTwin={activeTwin} />
                </div>
              </HoverTooltip>

              {/* Memory Health - Brain Scan */}
              <HoverTooltip
                title="Brain Scan (Memory Health)"
                description="Real-time visualization of Qdrant vector database health. Shows Recall Efficiency (HNSW index performance) and Fragmentation Map (segment statuses). Updates automatically when reindexing tasks run."
              >
                <div>
                  <MemoryHealth collectionName="agent_logs" refreshInterval={5000} />
                </div>
              </HoverTooltip>
           </div>
         </section>

        {/* DOMAIN CONFIDENCE GAUGES */}
        <section className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)]">
          <div className="flex items-center justify-between mb-4">
            <HoverTooltip
              title="Domain Confidence Gauges"
              description="Shows which knowledge domains (Mind/Body/Heart/Soul) are contributing to the current chat session. Higher percentages indicate stronger domain influence on agent responses."
            >
              <div className="flex items-center gap-2 cursor-help">
                <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--bg-steel)] rounded">
                  <ICONS.Brain />
                </div>
                <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">Domain Confidence</h3>
              </div>
            </HoverTooltip>
            <button
              onClick={() => setShowAttributionDetails(!showAttributionDetails)}
              className="px-2 py-1 text-[8px] bg-[rgb(var(--surface-rgb)/0.5)] hover:bg-[rgb(var(--surface-rgb)/0.7)] text-[var(--text-secondary)] rounded transition-all border border-[rgb(var(--bg-steel-rgb)/0.3)]"
              title={showAttributionDetails ? "Hide percentage details" : "Show percentage details"}
            >
              {showAttributionDetails ? 'Hide %' : 'Show %'}
            </button>
          </div>

          <div className="bg-[rgb(var(--surface-rgb)/0.3)] p-3 rounded-xl border border-[rgb(var(--bg-steel-rgb)/0.3)] space-y-3">
            {/* Mind Domain */}
            <HoverTooltip
              title="Mind (Intellectual) Domain"
              description="Technical specifications, logical procedures, playbooks, and verified code patterns. High values indicate technical/logical knowledge is driving responses."
            >
              <div className="space-y-1.5 cursor-help">
                  <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-[12px] text-[var(--bg-steel)]">psychology</span>
                    <span className="text-[9px] text-[var(--text-secondary)] font-bold">Mind</span>
                  </div>
                  {showAttributionDetails && (
                    <span className="text-[9px] text-[var(--bg-steel)] font-mono font-bold">
                      {domainAttribution ? Math.round(domainAttribution.mind) : 0}%
                    </span>
                  )}
                </div>
                <div className="h-1 bg-[rgb(var(--surface-rgb)/0.5)] rounded-full overflow-hidden">
                  <div 
                    className="h-full bg-[var(--bg-steel)] transition-all duration-500" 
                    style={{ width: `${domainAttribution ? domainAttribution.mind : 0}%` }} 
                  />
                </div>
              </div>
            </HoverTooltip>

            {/* Body Domain */}
            <HoverTooltip
              title="Body (Physical) Domain"
              description="Real-time telemetry data, system state, hardware metrics, and performance data. High values indicate physical/system state is driving responses."
            >
              <div className="space-y-1.5 cursor-help">
                  <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <ICONS.Activity />
                    <span className="text-[9px] text-[var(--text-secondary)] font-bold">Body</span>
                  </div>
                  {showAttributionDetails && (
                    <span className="text-[9px] text-[var(--bg-steel)] font-mono font-bold">
                      {domainAttribution ? Math.round(domainAttribution.body) : 0}%
                    </span>
                  )}
                </div>
                <div className="h-1 bg-[rgb(var(--surface-rgb)/0.5)] rounded-full overflow-hidden">
                  <div 
                    className="h-full bg-[var(--bg-steel)] transition-all duration-500" 
                    style={{ width: `${domainAttribution ? domainAttribution.body : 0}%` }} 
                  />
                </div>
              </div>
            </HoverTooltip>

            {/* Heart Domain */}
            <HoverTooltip
              title="Heart (Emotional) Domain"
              description="User preferences, agent personas, personalized context, and interaction history. High values indicate personalized alignment is driving responses."
            >
              <div className="space-y-1.5 cursor-help">
                  <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-[12px] text-[var(--bg-steel)]">favorite</span>
                    <span className="text-[9px] text-[var(--text-secondary)] font-bold">Heart</span>
                  </div>
                  {showAttributionDetails && (
                    <span className="text-[9px] text-[var(--bg-steel)] font-mono font-bold">
                      {domainAttribution ? Math.round(domainAttribution.heart) : 0}%
                    </span>
                  )}
                </div>
                <div className="h-1 bg-[rgb(var(--surface-rgb)/0.5)] rounded-full overflow-hidden">
                  <div 
                    className="h-full bg-[var(--bg-steel)] transition-all duration-500" 
                    style={{ width: `${domainAttribution ? domainAttribution.heart : 0}%` }} 
                  />
                </div>
              </div>
            </HoverTooltip>

            {/* Soul Domain */}
            <HoverTooltip
              title="Soul (Ethical) Domain"
              description="Corporate governance, leadership wisdom, audit trails, safety guardrails, and ethical guidelines. High values indicate ethical/governance constraints are driving responses."
            >
              <div className="space-y-1.5 cursor-help">
                  <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-[12px] text-[var(--bg-steel)]">shield</span>
                    <span className="text-[9px] text-[var(--text-secondary)] font-bold">Soul</span>
                  </div>
                  {showAttributionDetails && (
                    <span className="text-[9px] text-[var(--bg-steel)] font-mono font-bold">
                      {domainAttribution ? Math.round(domainAttribution.soul) : 0}%
                    </span>
                  )}
                </div>
                <div className="h-1 bg-[rgb(var(--surface-rgb)/0.5)] rounded-full overflow-hidden">
                  <div 
                    className="h-full bg-[var(--bg-steel)] transition-all duration-500" 
                    style={{ width: `${domainAttribution ? domainAttribution.soul : 0}%` }} 
                  />
                </div>
              </div>
            </HoverTooltip>
          </div>
        </section>

        {/* GLOBAL MISSION STATUS */}
        <section className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)]">
          <HoverTooltip
            title="Global Mission Status"
            description="High-level mission readiness indicators. These are dashboard signals intended to summarize system posture at a glance."
          >
            <div className="flex items-center gap-2 mb-4 cursor-help">
              <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--bg-steel)] rounded">
                <span className="material-symbols-outlined text-[14px]">shield</span>
              </div>
              <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">Global Mission Status</h3>
            </div>
          </HoverTooltip>

          <div className="bg-[rgb(var(--surface-rgb)/0.3)] p-3 rounded-xl border border-[rgb(var(--bg-steel-rgb)/0.3)] space-y-4">
            <HoverTooltip
              title="Neural Sync"
              description="Represents how synchronized the system’s memory/agent state is across components. Higher is better; drops may indicate delayed ingestion or connectivity issues."
            >
              <div className="space-y-2 cursor-help">
                <div className="flex justify-between text-[9px] text-[var(--text-secondary)]">
                  <span>Neural Sync</span>
                  <span className="text-[var(--bg-steel)] font-mono font-bold">{neuralSync}%</span>
                </div>
                <div className="h-1 bg-[rgb(var(--surface-rgb)/0.5)] rounded-full overflow-hidden" title="Neural Sync progress bar">
                  <div className="h-full bg-[var(--bg-steel)] transition-all duration-500" style={{ width: `${neuralSync}%` }} />
                </div>
              </div>
            </HoverTooltip>

            <HoverTooltip
              title="Threat Suppression"
              description="Represents progress toward containment/mitigation objectives for active threats (detections, patches, quarantines). Higher is better."
            >
              <div className="space-y-2 cursor-help">
                <div className="flex justify-between text-[9px] text-[var(--text-secondary)]">
                  <span>Threat Suppression</span>
                  <span className="text-[var(--bg-muted)] font-mono font-bold">72%</span>
                </div>
                <div className="h-1 bg-[rgb(var(--surface-rgb)/0.5)] rounded-full overflow-hidden" title="Threat Suppression progress bar">
                  <div className="h-full bg-[var(--bg-muted)] w-[72%]" />
                </div>
              </div>
            </HoverTooltip>
          </div>
        </section>

        {/* INGESTION PROGRESS DASHBOARD */}
        <section className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)]">
          <IngestorDashboard />
        </section>

        {/* ACTIVE JOBS */}
         <section className="p-4">
            <div className="flex items-center gap-2 mb-4">
              <HoverTooltip
                title="Mission Pipeline"
                description="Recent and active jobs emitted by the Orchestrator. Click a job to open logs and inspect actions taken."
              >
                 <span className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">Mission Pipeline</span>
               </HoverTooltip>
            </div>
            
            <div className="space-y-3">
              {jobs.length === 0 ? (
                <div className="text-[10px] text-[var(--text-muted)] text-center py-4 italic">No tactical missions executing</div>
              ) : (
                jobs.map(job => (
                  <HoverTooltip
                    key={job.id}
                    title={`Job: ${job.name}`}
                    description={`Status: ${job.status}. Progress: ${job.progress}%. Click to view logs for job_id=${job.id}.`}
                  >
                    <button 
                      onClick={() => onViewLogs(job.id)}
                      className={`w-full text-left bg-[rgb(var(--surface-rgb)/0.28)] p-2.5 rounded-xl border transition-all ${job.status === 'active' ? 'border-[rgb(var(--accent-rgb)/0.3)]' : 'border-[rgb(var(--bg-steel-rgb)/0.25)]'} hover:bg-[rgb(var(--surface-rgb)/0.4)]`}
                    >
                      <div className="flex justify-between items-start mb-2">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 mb-0.5">
                            <div className={`w-1.5 h-1.5 rounded-full ${getStatusBg(job.status)}`}></div>
                            <span className="text-[10px] font-bold text-[var(--text-primary)] truncate">{job.name}</span>
                          </div>
                        </div>
                        <div className={`px-1.5 py-0.5 rounded text-[7px] font-black uppercase tracking-widest border border-current ${getStatusColor(job.status)} bg-current/5`}>
                          {job.status}
                        </div>
                      </div>
                      <div className="h-1 bg-[rgb(var(--overlay-rgb)/0.2)] rounded-full overflow-hidden">
                        <div className={`h-full ${getStatusBg(job.status)} transition-all duration-500`} style={{ width: `${job.progress}%` }} />
                      </div>
                    </button>
                  </HoverTooltip>
                ))
              )}
           </div>
        </section>
      </div>
    </aside>
  );
};

export default SidebarRight;
