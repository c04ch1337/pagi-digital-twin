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
      case 'plan': return 'text-[#90C3EA] bg-[#90C3EA]/10 border-[#90C3EA]/20';
      case 'tool': return 'text-[#78A2C2] bg-[#78A2C2]/10 border-[#78A2C2]/20';
      case 'memory': return 'text-[#5381A5] bg-[#5381A5]/10 border-[#5381A5]/20';
      case 'error': return 'text-[#163247] bg-[#163247]/10 border-[#163247]/20';
      case 'warn': return 'text-[#78A2C2] bg-[#78A2C2]/10 border-[#78A2C2]/20';
      default: return 'text-[#163247] bg-[#163247]/10 border-[#163247]/20';
    }
  };

  const getStatusColor = (status: Job['status']) => {
    switch (status) {
      case 'completed': return 'text-[#5381A5]';
      case 'failed': return 'text-[#163247]';
      case 'active': return 'text-[#5381A5]';
      default: return 'text-[#163247]';
    }
  };

  return (
    <div className="flex-1 flex flex-col bg-[#0b1b2b] overflow-hidden font-mono">
      {/* Terminal Header */}
      <header className="h-12 border-b border-[#5381A5]/30 bg-[#163247] flex items-center justify-between px-4 shrink-0">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-[#78A2C2] text-sm">terminal</span>
            <h2 className="text-[10px] font-bold uppercase tracking-[0.2em] text-[#90C3EA]">
              Execution Logs: <span className="text-[#9EC9D9]">{job.name}</span>
            </h2>
          </div>
          <div className="h-4 w-px bg-[#5381A5]/30"></div>
          <div className="flex items-center gap-2">
            <span className="text-[9px] uppercase font-bold text-[#78A2C2] tracking-widest">Agent:</span>
            <span className="text-[9px] uppercase font-bold text-[#90C3EA] tracking-widest">{twin.name}</span>
          </div>
        </div>

        <div className="flex items-center gap-4">
           <div className="flex items-center gap-2">
             <span className={`w-1.5 h-1.5 rounded-full ${job.status === 'active' ? 'bg-[#5381A5] animate-pulse shadow-[0_0_8px_rgba(83,129,165,0.5)]' : 'bg-[#163247]'}`}></span>
             <span className={`text-[10px] font-bold uppercase tracking-widest ${getStatusColor(job.status)}`}>
               {job.status}
             </span>
           </div>
           <button 
             onClick={onClose}
             className="text-[#78A2C2] hover:text-[#90C3EA] transition-colors"
           >
             <span className="material-symbols-outlined text-sm">close</span>
           </button>
        </div>
      </header>

      {/* Log Body */}
      <div 
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 space-y-1 custom-scrollbar selection:bg-[#5381A5]/30 bg-[#0b1b2b]"
      >
        {job.logs?.map((log) => (
          <div key={log.id} className="flex gap-4 group hover:bg-[#163247]/30 transition-colors py-0.5 px-1 rounded">
            <span className="text-[#78A2C2] text-[10px] shrink-0 w-20">
              [{log.timestamp.toLocaleTimeString([], { hour12: false })}]
            </span>
            <span className={`text-[9px] px-1.5 py-0.5 rounded border uppercase font-bold tracking-tighter shrink-0 w-16 text-center ${getLevelStyle(log.level)}`}>
              {log.level}
            </span>
            <span className="text-[#90C3EA] text-[11px] leading-relaxed break-all">
              {log.message}
            </span>
          </div>
        ))}
        {job.status === 'active' && (
          <div className="flex gap-4 py-1 px-1">
             <span className="text-[#78A2C2] text-[10px] shrink-0 w-20">
               [{new Date().toLocaleTimeString([], { hour12: false })}]
             </span>
             <div className="flex items-center gap-2">
                <div className="w-1 h-3 bg-[#5381A5] animate-pulse"></div>
                <span className="text-[#90C3EA] text-[11px] font-bold animate-pulse italic">
                  Awaiting next sequence from {twin.name}...
                </span>
             </div>
          </div>
        )}
      </div>

      {/* Terminal Footer / Stats */}
      <footer className="h-10 border-t border-[#5381A5]/30 bg-[#163247] flex items-center justify-between px-4 shrink-0 overflow-hidden">
        <div className="flex gap-6 items-center">
           <div className="flex items-center gap-2">
             <span className="text-[9px] font-bold text-[#78A2C2] uppercase">Memory Allocation</span>
             <div className="w-24 h-1 bg-[#0b1b2b] rounded-full overflow-hidden">
                <div className="h-full bg-[#5381A5] w-[42%]"></div>
             </div>
             <span className="text-[9px] mono text-[#78A2C2]">421MB / 1GB</span>
           </div>
           <div className="flex items-center gap-2">
             <span className="text-[9px] font-bold text-[#78A2C2] uppercase">Context Health</span>
             <div className="w-24 h-1 bg-[#0b1b2b] rounded-full overflow-hidden">
                <div className="h-full bg-[#5381A5] w-[89%]"></div>
             </div>
             <span className="text-[9px] mono text-[#78A2C2]">89%</span>
           </div>
        </div>
        
        <div className="text-[9px] text-[#78A2C2] uppercase tracking-widest font-bold">
           Process ID: <span className="text-[#90C3EA]">{job.id.substring(0, 8)}</span>
        </div>
      </footer>
    </div>
  );
};

export default JobLogsView;
