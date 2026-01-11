import React, { useState, useEffect } from 'react';
import { TelemetryData, Job, Approval, Twin } from '../types';
import { ICONS } from '../constants';
import TelemetryCharts from './TelemetryCharts';
import { fetchNamespaceMetrics, MemoryStatus, VectorShard } from '../services/memory';
import { querySemanticMemory } from '../services/gemini';

interface SidebarRightProps {
  telemetry: TelemetryData[];
  jobs: Job[];
  approvals: Approval[];
  onApprove: (id: string) => void;
  onDeny: (id: string) => void;
  activeTwin: Twin;
  onViewLogs: (jobId: string) => void;
}

const SidebarRight: React.FC<SidebarRightProps> = ({ telemetry, jobs, approvals, onApprove, onDeny, activeTwin, onViewLogs }) => {
  const [memoryInfo, setMemoryInfo] = useState<MemoryStatus | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<VectorShard[]>([]);
  const [isSearching, setIsSearching] = useState(false);

  const latest = telemetry[telemetry.length - 1] || { cpu: 0, memory: 0, network: 0, gpu: 0 };

  useEffect(() => {
    const updateMemory = () => {
      const metrics = fetchNamespaceMetrics(activeTwin.settings.memoryNamespace);
      setMemoryInfo(metrics);
    };
    
    updateMemory();
    const interval = setInterval(updateMemory, 2000);
    return () => clearInterval(interval);
  }, [activeTwin.settings.memoryNamespace]);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery.trim()) return;

    setIsSearching(true);
    try {
      const results = await querySemanticMemory(activeTwin.settings.memoryNamespace, searchQuery);
      setSearchResults(results);
    } catch (err) {
      console.error(err);
    } finally {
      setIsSearching(false);
    }
  };

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
    <div className="absolute bottom-full mb-2 left-1/2 -translate-x-1/2 w-48 p-2 bg-zinc-900 border border-zinc-800 rounded shadow-2xl pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity z-50 text-left">
      <div className="text-[10px] font-bold text-indigo-400 uppercase tracking-widest mb-1">{title}</div>
      <div className="text-[9px] text-zinc-400 leading-tight">{description}</div>
      <div className="absolute top-full left-1/2 -translate-x-1/2 border-8 border-transparent border-t-zinc-800"></div>
    </div>
  );

  return (
    <aside className="w-80 bg-zinc-950 border-l border-zinc-800/50 flex flex-col shrink-0 relative">
      <div className="flex items-center h-14 border-b border-zinc-800/50">
        <div className="flex-1 flex justify-center border-r border-zinc-800/50 py-3 text-zinc-400 hover:text-white transition-colors cursor-pointer group relative">
          <div className="flex flex-col items-center">
            <ICONS.Brain />
            <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Mind</span>
          </div>
        </div>
        <div className="flex-1 flex justify-center border-r border-zinc-800/50 py-3 text-zinc-400 hover:text-white transition-colors cursor-pointer group relative">
          <div className="flex flex-col items-center">
            <ICONS.Heart />
            <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Heart</span>
          </div>
        </div>
        <div className="flex-1 flex justify-center py-3 text-zinc-400 hover:text-white transition-colors cursor-pointer group relative">
          <div className="flex flex-col items-center">
            <ICONS.Activity />
            <span className="text-[8px] uppercase font-bold mt-1 tracking-tighter opacity-0 group-hover:opacity-100 transition-opacity">Body</span>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {/* BODY - SYSTEM TELEMETRY */}
        <section className="p-4 border-b border-zinc-900">
          <div className="flex items-center justify-between mb-4">
             <div className="flex items-center gap-2">
                <div className="p-1.5 bg-emerald-500/10 text-emerald-500 rounded">
                   <ICONS.Activity />
                </div>
                <h3 className="text-xs font-bold uppercase tracking-widest text-zinc-500">Body (System)</h3>
             </div>
             <span className="text-[10px] text-zinc-600 mono">LAT: 24ms</span>
          </div>
          <div className="space-y-4">
             <div className="bg-zinc-900/50 rounded-xl p-3 border border-zinc-800/50">
                <TelemetryCharts data={telemetry} />
             </div>
             
             <div className="grid grid-cols-2 gap-2">
                <div className="bg-zinc-900/30 p-2 rounded-lg border border-zinc-800/50">
                   <div className="text-[9px] font-bold text-zinc-500 mb-1 uppercase tracking-tighter">CPU LOAD</div>
                   <div className="text-sm font-bold mono text-zinc-100">{latest.cpu}%</div>
                </div>
                <div className="bg-zinc-900/30 p-2 rounded-lg border border-zinc-800/50">
                   <div className="text-[9px] font-bold text-zinc-500 mb-1 uppercase tracking-tighter">RAM CORE</div>
                   <div className="text-sm font-bold mono text-zinc-100">{latest.memory}%</div>
                </div>
             </div>
          </div>
        </section>

        {/* MIND - VECTOR VAULT & SEMANTIC SEARCH */}
        <section className="p-4 border-b border-zinc-900">
          <div className="flex items-center gap-2 mb-4">
             <div className="p-1.5 bg-indigo-500/10 text-indigo-500 rounded">
                <ICONS.Brain />
             </div>
             <h3 className="text-xs font-bold uppercase tracking-widest text-zinc-500">Mind (Vector Vault)</h3>
          </div>

          <div className="space-y-4">
             {/* Namespace Status */}
             <div className="p-3 bg-zinc-900/30 rounded-xl border border-zinc-800/50">
                <div className="flex justify-between items-center mb-2">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-[12px] text-indigo-500">database</span>
                    <div className="text-[10px] font-bold text-zinc-400 uppercase tracking-widest">{activeTwin.settings.memoryNamespace}</div>
                  </div>
                  <span className="text-[9px] font-mono text-indigo-400 font-bold">{memoryInfo?.shardCount || 0} Shards</span>
                </div>
                <div className="flex gap-1 h-1.5 mb-1">
                   {Array.from({length: 12}).map((_, i) => (
                     <div key={i} className={`flex-1 rounded-sm transition-all duration-300 ${i < Math.ceil((memoryInfo?.load || 0) / 8.33) ? 'bg-indigo-500' : 'bg-zinc-800'}`} />
                   ))}
                </div>
             </div>

             {/* Semantic Search UI */}
             <form onSubmit={handleSearch} className="relative">
                <input 
                  value={searchQuery}
                  onChange={e => setSearchQuery(e.target.value)}
                  placeholder="Query semantic index..."
                  className="w-full bg-zinc-900 border border-zinc-800 rounded-lg pl-8 pr-3 py-2 text-[11px] focus:ring-1 focus:ring-indigo-500 outline-none transition-all placeholder-zinc-600"
                />
                <span className="material-symbols-outlined absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-600 text-[14px]">search</span>
                {isSearching && (
                  <div className="absolute right-2 top-1/2 -translate-y-1/2 w-3 h-3 border border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
                )}
             </form>

             {/* Results */}
             <div className="space-y-2">
                {searchResults.length > 0 ? (
                  searchResults.map(shard => (
                    <div key={shard.id} className="p-2.5 bg-zinc-900/50 border border-zinc-800 rounded-lg group hover:border-indigo-500/50 transition-all">
                       <div className="flex items-center justify-between mb-1.5">
                          <span className="text-[8px] font-bold text-indigo-500 uppercase tracking-widest">Matched Shard</span>
                          <span className="text-[8px] text-zinc-600 font-mono">{shard.timestamp.toLocaleTimeString()}</span>
                       </div>
                       <p className="text-[10px] text-zinc-300 leading-snug line-clamp-3">{shard.text}</p>
                    </div>
                  ))
                ) : searchQuery && !isSearching ? (
                  <div className="text-[9px] text-zinc-600 text-center py-2 italic">No semantic matches in this namespace.</div>
                ) : null}
             </div>
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
