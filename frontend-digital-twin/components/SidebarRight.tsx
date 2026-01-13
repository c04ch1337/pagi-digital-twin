import React, { useState, useEffect } from 'react';
import { Job, Approval, Twin } from '../types';
import { ICONS } from '../constants';
import TelemetryCharts from './TelemetryCharts';
import NeuralMemorySearch from './NeuralMemorySearch';
import HoverTooltip from './HoverTooltip';
import { fetchNamespaceMetrics, MemoryStatus } from '../services/memory';
import { useTelemetry } from '../context/TelemetryContext';
import { fetchSyncMetrics } from '../services/systemService';

interface SidebarRightProps {
  jobs: Job[];
  approvals: Approval[];
  onApprove: (id: string) => void;
  onDeny: (id: string) => void;
  activeTwin: Twin;
  onViewLogs: (jobId: string) => void;
}

const SidebarRight: React.FC<SidebarRightProps> = ({ jobs, approvals, onApprove, onDeny, activeTwin, onViewLogs }) => {
  // Get telemetry data from context (SSE stream)
  const { telemetry, isConnected: isTelemetryConnected } = useTelemetry();
  const [memoryInfo, setMemoryInfo] = useState<MemoryStatus | null>(null);
  const [neuralSync, setNeuralSync] = useState<number>(100);

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
    const base = 'bg-white/30 p-2 rounded-lg border transition-colors';
    const offlineMute = offline ? ' opacity-60' : '';

    switch (severity) {
      case 'critical':
        return `${base} border-rose-500/60 bg-rose-200/30 ${offline ? '' : 'animate-pulse'} ring-1 ring-rose-500/20${offlineMute}`;
      case 'high':
        return `${base} border-amber-500/50 bg-amber-200/25 ${offline ? '' : 'animate-pulse'}${offlineMute}`;
      case 'normal':
      default:
        return `${base} border-[#5381A5]/30${offlineMute}`;
    }
  };

  const metricValueClasses = (severity: MetricSeverity): string => {
    switch (severity) {
      case 'critical':
        return 'text-rose-700';
      case 'high':
        return 'text-amber-800';
      case 'normal':
      default:
        return 'text-[#0b1b2b]';
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
      case 'completed': return 'text-[#5381A5]';
      case 'failed': return 'text-[#163247]';
      case 'active': return 'text-[#5381A5]';
      case 'pending': return 'text-[#78A2C2]';
      default: return 'text-[#163247]';
    }
  };

  const getStatusBg = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'bg-[#5381A5]';
      case 'failed': return 'bg-[#163247]';
      case 'active': return 'bg-[#5381A5]';
      case 'pending': return 'bg-[#78A2C2]';
      default: return 'bg-[#163247]';
    }
  };

  return (
    <aside className="w-80 bg-[#90C3EA] border-l border-[#5381A5]/30 flex flex-col shrink-0 relative">
      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {/* BODY - SYSTEM TELEMETRY */}
        <section className="p-4 border-b border-[#5381A5]/20">
          <div className="flex items-center justify-between mb-4">
             <HoverTooltip
               title="System Telemetry"
               description="Live hardware telemetry (CPU, memory, and network) streamed from the Telemetry service and displayed as recent time-series samples."
             >
               <div className="flex items-center gap-2">
                 <div className="p-1.5 bg-white/40 text-[#5381A5] rounded">
                   <ICONS.Activity />
                 </div>
                 <h3 className="text-xs font-bold uppercase tracking-widest text-[#163247]">Body (System)</h3>
               </div>
             </HoverTooltip>

             <HoverTooltip
               title="Telemetry Link"
               description="Connection status to the telemetry stream (SSE). LIVE means the UI is receiving fresh samples; OFFLINE means telemetry updates are not arriving."
             >
               <div className="flex items-center gap-2">
                 <span className={`w-1.5 h-1.5 rounded-full ${isTelemetryConnected ? 'bg-[#5381A5] animate-pulse' : 'bg-[#78A2C2]'}`}></span>
                 <span className="text-[10px] text-[#163247] mono">
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
               <div className="bg-white/40 rounded-xl p-3 border border-[#5381A5]/30">
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
                          <div className="text-[9px] font-bold text-[#163247] uppercase tracking-tighter">CPU LOAD</div>
                          {metricLabel(sev) && (
                            <div
                              className={`text-[8px] font-black uppercase tracking-widest px-1.5 py-0.5 rounded border ${
                                sev === 'critical'
                                  ? 'text-rose-700 border-rose-500/40 bg-rose-200/30'
                                  : 'text-amber-800 border-amber-500/40 bg-amber-200/30'
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
                          <div className="text-[9px] font-bold text-[#163247] uppercase tracking-tighter">RAM CORE</div>
                          {metricLabel(sev) && (
                            <div
                              className={`text-[8px] font-black uppercase tracking-widest px-1.5 py-0.5 rounded border ${
                                sev === 'critical'
                                  ? 'text-rose-700 border-rose-500/40 bg-rose-200/30'
                                  : 'text-amber-800 border-amber-500/40 bg-amber-200/30'
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
        <section className="p-4 border-b border-[#5381A5]/20">
          <HoverTooltip
            title="Mind (Vector Vault)"
            description="Semantic memory storage + query surface for the active twin. Shows the current namespace health and provides a vector search UI."
          >
            <div className="flex items-center gap-2 mb-4 cursor-help">
              <div className="p-1.5 bg-white/40 text-[#5381A5] rounded">
                <ICONS.Brain />
              </div>
              <h3 className="text-xs font-bold uppercase tracking-widest text-[#163247]">Mind (Vector Vault)</h3>
            </div>
          </HoverTooltip>

           <div className="space-y-4">
              {/* Namespace Status */}
              <HoverTooltip
                title="Namespace Status"
                description="Active memory namespace for this twin (Vector Vault). Shards indicate storage partitions; the bar approximates current load/pressure for the namespace."
              >
                <div className="p-3 bg-white/30 rounded-xl border border-[#5381A5]/30 cursor-help">
                  <div className="flex justify-between items-center mb-2">
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-[12px] text-[#5381A5]">database</span>
                      <div className="text-[10px] font-bold text-[#0b1b2b] uppercase tracking-widest">{activeTwin.settings.memoryNamespace}</div>
                    </div>
                    <span className="text-[9px] font-mono text-[#5381A5] font-bold">{memoryInfo?.shardCount || 0} Shards</span>
                  </div>
                  <div className="flex gap-1 h-1.5 mb-1">
                    {Array.from({ length: 12 }).map((_, i) => (
                      <div
                        key={i}
                        className={`flex-1 rounded-sm transition-all duration-300 ${
                          i < Math.ceil((memoryInfo?.load || 0) / 8.33) ? 'bg-[#5381A5]' : 'bg-white/40'
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
           </div>
         </section>

        {/* GLOBAL MISSION STATUS */}
        <section className="p-4 border-b border-[#5381A5]/20">
          <HoverTooltip
            title="Global Mission Status"
            description="High-level mission readiness indicators. These are dashboard signals intended to summarize system posture at a glance."
          >
            <div className="flex items-center gap-2 mb-4 cursor-help">
              <div className="p-1.5 bg-white/40 text-[#5381A5] rounded">
                <span className="material-symbols-outlined text-[14px]">shield</span>
              </div>
              <h3 className="text-xs font-bold uppercase tracking-widest text-[#163247]">Global Mission Status</h3>
            </div>
          </HoverTooltip>

          <div className="bg-white/30 p-3 rounded-xl border border-[#5381A5]/30 space-y-4">
            <HoverTooltip
              title="Neural Sync"
              description="Represents how synchronized the system’s memory/agent state is across components. Higher is better; drops may indicate delayed ingestion or connectivity issues."
            >
              <div className="space-y-2 cursor-help">
                <div className="flex justify-between text-[9px] text-[#163247]">
                  <span>Neural Sync</span>
                  <span className="text-[#5381A5] font-mono font-bold">{neuralSync}%</span>
                </div>
                <div className="h-1 bg-white/50 rounded-full overflow-hidden" title="Neural Sync progress bar">
                  <div className="h-full bg-[#5381A5] transition-all duration-500" style={{ width: `${neuralSync}%` }} />
                </div>
              </div>
            </HoverTooltip>

            <HoverTooltip
              title="Threat Suppression"
              description="Represents progress toward containment/mitigation objectives for active threats (detections, patches, quarantines). Higher is better."
            >
              <div className="space-y-2 cursor-help">
                <div className="flex justify-between text-[9px] text-[#163247]">
                  <span>Threat Suppression</span>
                  <span className="text-[#78A2C2] font-mono font-bold">72%</span>
                </div>
                <div className="h-1 bg-white/50 rounded-full overflow-hidden" title="Threat Suppression progress bar">
                  <div className="h-full bg-[#78A2C2] w-[72%]" />
                </div>
              </div>
            </HoverTooltip>
          </div>
        </section>

        {/* ACTIVE JOBS */}
        <section className="p-4">
           <div className="flex items-center gap-2 mb-4">
              <HoverTooltip
                title="Mission Pipeline"
                description="Recent and active jobs emitted by the Orchestrator. Click a job to open logs and inspect actions taken."
              >
                <span className="text-xs font-bold uppercase tracking-widest text-zinc-500">Mission Pipeline</span>
              </HoverTooltip>
           </div>
           
           <div className="space-y-3">
              {jobs.length === 0 ? (
                <div className="text-[10px] text-zinc-600 text-center py-4 italic">No tactical missions executing</div>
              ) : (
                jobs.map(job => (
                  <HoverTooltip
                    key={job.id}
                    title={`Job: ${job.name}`}
                    description={`Status: ${job.status}. Progress: ${job.progress}%. Click to view logs for job_id=${job.id}.`}
                  >
                    <button 
                      onClick={() => onViewLogs(job.id)}
                      className={`w-full text-left bg-zinc-900/40 p-2.5 rounded-xl border transition-all ${job.status === 'active' ? 'border-indigo-500/30' : 'border-zinc-800'} hover:bg-zinc-900`}
                    >
                      <div className="flex justify-between items-start mb-2">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 mb-0.5">
                            <div className={`w-1.5 h-1.5 rounded-full ${getStatusBg(job.status)}`}></div>
                            <span className="text-[10px] font-bold text-zinc-200 truncate">{job.name}</span>
                          </div>
                        </div>
                        <div className={`px-1.5 py-0.5 rounded text-[7px] font-black uppercase tracking-widest border border-current ${getStatusColor(job.status)} bg-current/5`}>
                          {job.status}
                        </div>
                      </div>
                      <div className="h-1 bg-zinc-950 rounded-full overflow-hidden">
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
