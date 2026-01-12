import React, { useState, useEffect } from 'react';
import { Job, Approval, Twin } from '../types';
import { ICONS } from '../constants';
import TelemetryCharts from './TelemetryCharts';
import NeuralMemorySearch from './NeuralMemorySearch';
import HoverTooltip from './HoverTooltip';
import { fetchNamespaceMetrics, MemoryStatus } from '../services/memory';
import { useTelemetry } from '../context/TelemetryContext';

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

  const latest = telemetry.length > 0 ? telemetry[telemetry.length - 1] : { cpu: 0, memory: 0, network: 0, gpu: 0, timestamp: '' };

  useEffect(() => {
    const updateMemory = () => {
      const metrics = fetchNamespaceMetrics(activeTwin.settings.memoryNamespace);
      setMemoryInfo(metrics);
    };
    
    updateMemory();
    const interval = setInterval(updateMemory, 2000);
    return () => clearInterval(interval);
  }, [activeTwin.settings.memoryNamespace]);


  const getStatusColor = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'text-emerald-400';
      case 'failed': return 'text-rose-400';
      case 'active': return 'text-indigo-400';
      case 'pending': return 'text-amber-400';
      default: return 'text-zinc-500';
    }
  };

  const getStatusBg = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'bg-emerald-500';
      case 'failed': return 'bg-rose-500';
      case 'active': return 'bg-indigo-500';
      case 'pending': return 'bg-amber-500';
      default: return 'bg-zinc-500';
    }
  };

  return (
    <aside className="w-80 bg-[#90C3EA] border-l border-[#5381A5]/30 flex flex-col shrink-0 relative">
      <div className="flex items-center h-14 border-b border-[#5381A5]/30">
        <HoverTooltip
          title="Mind"
          description="Vector vault indicators and semantic memory query. Shows the active namespace and lets you search stored intelligence."
        >
          <div className="flex-1 flex justify-center border-r border-[#5381A5]/30 py-3 text-[#163247] hover:text-[#0b1b2b] transition-colors cursor-pointer group">
            <div className="flex flex-col items-center">
              <ICONS.Brain />
              <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Mind</span>
            </div>
          </div>
        </HoverTooltip>

        <HoverTooltip
          title="Heart"
          description="Operator-facing status/health area. Reserved for trust, approvals, and other human-in-the-loop controls."
        >
          <div className="flex-1 flex justify-center border-r border-[#5381A5]/30 py-3 text-[#163247] hover:text-[#0b1b2b] transition-colors cursor-pointer group">
            <div className="flex flex-col items-center">
              <ICONS.Heart />
              <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Heart</span>
            </div>
          </div>
        </HoverTooltip>

        <HoverTooltip
          title="Body"
          description="System telemetry view. Shows CPU, memory, and network time-series along with the latest values."
        >
          <div className="flex-1 flex justify-center py-3 text-[#163247] hover:text-[#0b1b2b] transition-colors cursor-pointer group">
            <div className="flex flex-col items-center">
              <ICONS.Activity />
              <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Body</span>
            </div>
          </div>
        </HoverTooltip>
      </div>

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
                  <div className="bg-white/30 p-2 rounded-lg border border-[#5381A5]/30">
                    <div className="text-[9px] font-bold text-[#163247] mb-1 uppercase tracking-tighter">CPU LOAD</div>
                    <div className="text-sm font-bold mono text-[#0b1b2b]">{Number(latest.cpu).toFixed(2)}%</div>
                  </div>
                </HoverTooltip>

                <HoverTooltip
                  title="RAM Core"
                  description="Current memory utilization (percent). High values can indicate memory pressure and may impact tool execution performance."
                >
                  <div className="bg-white/30 p-2 rounded-lg border border-[#5381A5]/30">
                    <div className="text-[9px] font-bold text-[#163247] mb-1 uppercase tracking-tighter">RAM CORE</div>
                    <div className="text-sm font-bold mono text-[#0b1b2b]">{Number(latest.memory).toFixed(2)}%</div>
                  </div>
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
