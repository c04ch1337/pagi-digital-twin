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
      case 'plan': return 'text-[var(--bg-secondary)] bg-[rgb(var(--bg-secondary-rgb)/0.1)] border-[rgb(var(--bg-secondary-rgb)/0.2)]';
      case 'tool': return 'text-[var(--bg-muted)] bg-[rgb(var(--bg-muted-rgb)/0.1)] border-[rgb(var(--bg-muted-rgb)/0.2)]';
      case 'memory': return 'text-[var(--bg-steel)] bg-[rgb(var(--bg-steel-rgb)/0.1)] border-[rgb(var(--bg-steel-rgb)/0.2)]';
      case 'error': return 'text-[var(--text-secondary)] bg-[rgb(var(--text-secondary-rgb)/0.1)] border-[rgb(var(--text-secondary-rgb)/0.2)]';
      case 'warn': return 'text-[var(--bg-muted)] bg-[rgb(var(--bg-muted-rgb)/0.1)] border-[rgb(var(--bg-muted-rgb)/0.2)]';
      default: return 'text-[var(--text-secondary)] bg-[rgb(var(--text-secondary-rgb)/0.1)] border-[rgb(var(--text-secondary-rgb)/0.2)]';
    }
  };

  const getStatusColor = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'text-[var(--bg-steel)]';
      case 'failed': return 'text-[var(--text-secondary)]';
      case 'active': return 'text-[var(--bg-steel)]';
      default: return 'text-[var(--text-secondary)]';
    }
  };

  return (
    <div className="flex-1 flex flex-col bg-[var(--text-primary)] overflow-hidden font-mono">
      {/* Terminal Header */}
      <header className="h-12 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--text-secondary)] flex items-center justify-between px-4 shrink-0">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-[var(--bg-muted)] text-sm">terminal</span>
            <h2 className="text-[10px] font-bold uppercase tracking-[0.2em] text-[var(--bg-secondary)]">
              Execution Logs: <span className="text-[var(--bg-primary)]">{job.name}</span>
            </h2>
          </div>
          <div className="h-4 w-px bg-[rgb(var(--bg-steel-rgb)/0.3)]"></div>
          <div className="flex items-center gap-2">
            <span className="text-[9px] uppercase font-bold text-[var(--bg-muted)] tracking-widest">Agent:</span>
            <span className="text-[9px] uppercase font-bold text-[var(--bg-secondary)] tracking-widest">{twin.name}</span>
          </div>
        </div>

        <div className="flex items-center gap-4">
           <div className="flex items-center gap-2">
             <span className={`w-1.5 h-1.5 rounded-full ${job.status === 'active' ? 'bg-[var(--bg-steel)] animate-pulse shadow-[0_0_8px_rgb(var(--bg-steel-rgb)/0.5)]' : 'bg-[var(--text-secondary)]'}`}></span>
             <span className={`text-[10px] font-bold uppercase tracking-widest ${getStatusColor(job.status)}`}>
               {job.status}
             </span>
           </div>
           <button 
             onClick={onClose}
             className="text-[var(--bg-muted)] hover:text-[var(--bg-secondary)] transition-colors"
           >
             <span className="material-symbols-outlined text-sm">close</span>
           </button>
        </div>
      </header>

      {/* Log Body */}
      <div 
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 space-y-1 custom-scrollbar selection:bg-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--text-primary)]"
      >
        {job.logs?.map((log) => (
          <div key={log.id} className="flex gap-4 group hover:bg-[rgb(var(--text-secondary-rgb)/0.3)] transition-colors py-0.5 px-1 rounded">
            <span className="text-[var(--bg-muted)] text-[10px] shrink-0 w-20">
              [{log.timestamp.toLocaleTimeString([], { hour12: false })}]
            </span>
            <span className={`text-[9px] px-1.5 py-0.5 rounded border uppercase font-bold tracking-tighter shrink-0 w-16 text-center ${getLevelStyle(log.level)}`}>
              {log.level}
            </span>
            <span className="text-[var(--bg-secondary)] text-[11px] leading-relaxed break-all">
              {log.message}
            </span>
          </div>
        ))}
        {job.status === 'active' && (
          <div className="flex gap-4 py-1 px-1">
             <span className="text-[var(--bg-muted)] text-[10px] shrink-0 w-20">
               [{new Date().toLocaleTimeString([], { hour12: false })}]
             </span>
             <div className="flex items-center gap-2">
                <div className="w-1 h-3 bg-[var(--bg-steel)] animate-pulse"></div>
                <span className="text-[var(--bg-secondary)] text-[11px] font-bold animate-pulse italic">
                  Awaiting next sequence from {twin.name}...
                </span>
             </div>
          </div>
        )}
      </div>

      {/* Terminal Footer / Stats */}
      <footer className="h-10 border-t border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--text-secondary)] flex items-center justify-between px-4 shrink-0 overflow-hidden">
        <div className="flex gap-6 items-center">
           <div className="flex items-center gap-2">
             <span className="text-[9px] font-bold text-[var(--bg-muted)] uppercase">Memory Allocation</span>
             <div className="w-24 h-1 bg-[var(--text-primary)] rounded-full overflow-hidden">
                <div className="h-full bg-[var(--bg-steel)] w-[42%]"></div>
             </div>
             <span className="text-[9px] mono text-[var(--bg-muted)]">421MB / 1GB</span>
           </div>
           <div className="flex items-center gap-2">
             <span className="text-[9px] font-bold text-[var(--bg-muted)] uppercase">Context Health</span>
             <div className="w-24 h-1 bg-[var(--text-primary)] rounded-full overflow-hidden">
                <div className="h-full bg-[var(--bg-steel)] w-[89%]"></div>
             </div>
             <span className="text-[9px] mono text-[var(--bg-muted)]">89%</span>
           </div>
        </div>
        
        <div className="text-[9px] text-[var(--bg-muted)] uppercase tracking-widest font-bold">
           Process ID: <span className="text-[var(--bg-secondary)]">{job.id.substring(0, 8)}</span>
        </div>
      </footer>
    </div>
  );
};

export default JobLogsView;
