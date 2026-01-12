import React, { useState, useEffect } from 'react';
import { Job, Approval, Twin } from '../types';
import { ICONS } from '../constants';
import TelemetryCharts from './TelemetryCharts';
import NeuralMemorySearch from './NeuralMemorySearch';
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

  const Tooltip = ({ title, description }: { title: string, description: string }) => (
    <div className="absolute bottom-full mb-2 left-1/2 -translate-x-1/2 w-48 p-2 bg-white/80 border border-[#5381A5]/30 rounded shadow-2xl pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity z-50 text-left">
      <div className="text-[10px] font-bold text-[#5381A5] uppercase tracking-widest mb-1">{title}</div>
      <div className="text-[9px] text-[#163247] leading-tight">{description}</div>
      <div className="absolute top-full left-1/2 -translate-x-1/2 border-8 border-transparent border-t-[#5381A5]/30"></div>
    </div>
  );

  return (
    <aside className="w-80 bg-[#90C3EA] border-l border-[#5381A5]/30 flex flex-col shrink-0 relative">
      <div className="flex items-center h-14 border-b border-[#5381A5]/30">
        <div className="flex-1 flex justify-center border-r border-[#5381A5]/30 py-3 text-[#163247] hover:text-[#0b1b2b] transition-colors cursor-pointer group relative">
          <div className="flex flex-col items-center">
            <ICONS.Brain />
            <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Mind</span>
          </div>
        </div>
        <div className="flex-1 flex justify-center border-r border-[#5381A5]/30 py-3 text-[#163247] hover:text-[#0b1b2b] transition-colors cursor-pointer group relative">
          <div className="flex flex-col items-center">
            <ICONS.Heart />
            <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Heart</span>
          </div>
        </div>
        <div className="flex-1 flex justify-center py-3 text-[#163247] hover:text-[#0b1b2b] transition-colors cursor-pointer group relative">
          <div className="flex flex-col items-center">
            <ICONS.Activity />
            <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Body</span>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {/* BODY - SYSTEM TELEMETRY */}
        <section className="p-4 border-b border-[#5381A5]/20">
          <div className="flex items-center justify-between mb-4">
             <div className="flex items-center gap-2">
                <div className="p-1.5 bg-white/40 text-[#5381A5] rounded">
                   <ICONS.Activity />
                </div>
                <h3 className="text-xs font-bold uppercase tracking-widest text-[#163247]">Body (System)</h3>
             </div>
             <div className="flex items-center gap-2">
               <span className={`w-1.5 h-1.5 rounded-full ${isTelemetryConnected ? 'bg-[#5381A5] animate-pulse' : 'bg-[#78A2C2]'}`}></span>
               <span className="text-[10px] text-[#163247] mono">
                  {isTelemetryConnected ? 'LIVE' : 'OFFLINE'}
                </span>
              </div>
           </div>
           <div className="space-y-4">
             <div className="bg-white/40 rounded-xl p-3 border border-[#5381A5]/30">
                <TelemetryCharts data={telemetry} />
             </div>
             
             <div className="grid grid-cols-2 gap-2">
                <div className="bg-white/30 p-2 rounded-lg border border-[#5381A5]/30">
                   <div className="text-[9px] font-bold text-[#163247] mb-1 uppercase tracking-tighter">CPU LOAD</div>
                   <div className="text-sm font-bold mono text-[#0b1b2b]">{latest.cpu}%</div>
                </div>
                <div className="bg-white/30 p-2 rounded-lg border border-[#5381A5]/30">
                   <div className="text-[9px] font-bold text-[#163247] mb-1 uppercase tracking-tighter">RAM CORE</div>
                   <div className="text-sm font-bold mono text-[#0b1b2b]">{latest.memory}%</div>
                </div>
              </div>
           </div>
         </section>

        {/* MIND - VECTOR VAULT & SEMANTIC SEARCH */}
        <section className="p-4 border-b border-[#5381A5]/20">
          <div className="flex items-center gap-2 mb-4">
             <div className="p-1.5 bg-white/40 text-[#5381A5] rounded">
                <ICONS.Brain />
              </div>
             <h3 className="text-xs font-bold uppercase tracking-widest text-[#163247]">Mind (Vector Vault)</h3>
          </div>

          <div className="space-y-4">
             {/* Namespace Status */}
             <div className="p-3 bg-white/30 rounded-xl border border-[#5381A5]/30">
                <div className="flex justify-between items-center mb-2">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-[12px] text-[#5381A5]">database</span>
                    <div className="text-[10px] font-bold text-[#0b1b2b] uppercase tracking-widest">{activeTwin.settings.memoryNamespace}</div>
                  </div>
                  <span className="text-[9px] font-mono text-[#5381A5] font-bold">{memoryInfo?.shardCount || 0} Shards</span>
                </div>
                <div className="flex gap-1 h-1.5 mb-1">
                   {Array.from({length: 12}).map((_, i) => (
                     <div key={i} className={`flex-1 rounded-sm transition-all duration-300 ${i < Math.ceil((memoryInfo?.load || 0) / 8.33) ? 'bg-[#5381A5]' : 'bg-white/40'}`} />
                   ))}
                </div>
              </div>

             {/* Neural Memory Search */}
             <NeuralMemorySearch activeTwin={activeTwin} />
          </div>
        </section>

        {/* ACTIVE JOBS */}
        <section className="p-4">
           <div className="flex items-center gap-2 mb-4">
              <span className="text-xs font-bold uppercase tracking-widest text-zinc-500">Mission Pipeline</span>
           </div>
           
           <div className="space-y-3">
              {jobs.length === 0 ? (
                <div className="text-[10px] text-zinc-600 text-center py-4 italic">No tactical missions executing</div>
              ) : (
                jobs.map(job => (
                  <button 
                    key={job.id} 
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
                ))
              )}
           </div>
        </section>
      </div>
    </aside>
  );
};

export default SidebarRight;
