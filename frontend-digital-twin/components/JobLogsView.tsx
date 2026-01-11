import React, { useEffect, useRef } from 'react';
import { Job, Twin } from '../types';

interface JobLogsViewProps {
  job: Job;
  twin: Twin;
  onClose: () => void;
}

const JobLogsView: React.FC<JobLogsViewProps> = ({ job, twin, onClose }) => {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [job.logs]);

  const getLevelStyle = (level: string) => {
    switch (level) {
      case 'plan': return 'text-cyan-400 bg-cyan-400/10 border-cyan-400/20';
      case 'tool': return 'text-amber-400 bg-amber-400/10 border-amber-400/20';
      case 'memory': return 'text-purple-400 bg-purple-400/10 border-purple-400/20';
      case 'error': return 'text-rose-400 bg-rose-400/10 border-rose-400/20';
      case 'warn': return 'text-yellow-400 bg-yellow-400/10 border-yellow-400/20';
      default: return 'text-zinc-400 bg-zinc-400/10 border-zinc-400/20';
    }
  };

  const getStatusColor = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'text-emerald-400';
      case 'failed': return 'text-rose-400';
      case 'active': return 'text-indigo-400';
      default: return 'text-zinc-500';
    }
  };

  return (
    <div className="flex-1 flex flex-col bg-[#050507] overflow-hidden font-mono">
      {/* Terminal Header */}
      <header className="h-12 border-b border-zinc-800/50 bg-zinc-950 flex items-center justify-between px-4 shrink-0">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-zinc-500 text-sm">terminal</span>
            <h2 className="text-[10px] font-bold uppercase tracking-[0.2em] text-zinc-400">
              Execution Logs: <span className="text-zinc-100">{job.name}</span>
            </h2>
          </div>
          <div className="h-4 w-px bg-zinc-800"></div>
          <div className="flex items-center gap-2">
            <span className="text-[9px] uppercase font-bold text-zinc-600 tracking-widest">Agent:</span>
            <span className="text-[9px] uppercase font-bold text-indigo-400 tracking-widest">{twin.name}</span>
          </div>
        </div>

        <div className="flex items-center gap-4">
           <div className="flex items-center gap-2">
             <span className={`w-1.5 h-1.5 rounded-full ${job.status === 'active' ? 'bg-indigo-500 animate-pulse shadow-[0_0_8px_rgba(99,102,241,0.5)]' : 'bg-zinc-600'}`}></span>
             <span className={`text-[10px] font-bold uppercase tracking-widest ${getStatusColor(job.status)}`}>
               {job.status}
             </span>
           </div>
           <button 
             onClick={onClose}
             className="text-zinc-500 hover:text-white transition-colors"
           >
             <span className="material-symbols-outlined text-sm">close</span>
           </button>
        </div>
      </header>

      {/* Log Body */}
      <div 
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 space-y-1 custom-scrollbar selection:bg-indigo-500/30"
      >
        {job.logs?.map((log) => (
          <div key={log.id} className="flex gap-4 group hover:bg-zinc-900/30 transition-colors py-0.5 px-1 rounded">
            <span className="text-zinc-700 text-[10px] shrink-0 w-20">
              [{log.timestamp.toLocaleTimeString([], { hour12: false })}]
            </span>
            <span className={`text-[9px] px-1.5 py-0.5 rounded border uppercase font-bold tracking-tighter shrink-0 w-16 text-center ${getLevelStyle(log.level)}`}>
              {log.level}
            </span>
            <span className="text-zinc-300 text-[11px] leading-relaxed break-all">
              {log.message}
            </span>
          </div>
        ))}
        {job.status === 'active' && (
          <div className="flex gap-4 py-1 px-1">
             <span className="text-zinc-700 text-[10px] shrink-0 w-20">
               [{new Date().toLocaleTimeString([], { hour12: false })}]
             </span>
             <div className="flex items-center gap-2">
                <div className="w-1 h-3 bg-indigo-500 animate-pulse"></div>
                <span className="text-indigo-400 text-[11px] font-bold animate-pulse italic">
                  Awaiting next sequence from {twin.name}...
                </span>
             </div>
          </div>
        )}
      </div>

      {/* Terminal Footer / Stats */}
      <footer className="h-10 border-t border-zinc-800/50 bg-zinc-950 flex items-center justify-between px-4 shrink-0 overflow-hidden">
        <div className="flex gap-6 items-center">
           <div className="flex items-center gap-2">
             <span className="text-[9px] font-bold text-zinc-600 uppercase">Memory Allocation</span>
             <div className="w-24 h-1 bg-zinc-800 rounded-full overflow-hidden">
                <div className="h-full bg-emerald-500 w-[42%]"></div>
             </div>
             <span className="text-[9px] mono text-zinc-500">421MB / 1GB</span>
           </div>
           <div className="flex items-center gap-2">
             <span className="text-[9px] font-bold text-zinc-600 uppercase">Context Health</span>
             <div className="w-24 h-1 bg-zinc-800 rounded-full overflow-hidden">
                <div className="h-full bg-indigo-500 w-[89%]"></div>
             </div>
             <span className="text-[9px] mono text-zinc-500">89%</span>
           </div>
        </div>
        
        <div className="text-[9px] text-zinc-700 uppercase tracking-widest font-bold">
           Process ID: <span className="text-zinc-500">{job.id.substring(0, 8)}</span>
        </div>
      </footer>
    </div>
  );
};

export default JobLogsView;
