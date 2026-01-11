import React, { useState, useMemo } from 'react';
import { Message, Job, Twin, LogEntry } from '../types';

interface SearchViewProps {
  messages: Message[];
  jobs: Job[];
  twins: Twin[];
  onNavigateToChat: (twinId: string) => void;
  onNavigateToLogs: (jobId: string) => void;
  onClose: () => void;
}

const SearchView: React.FC<SearchViewProps> = ({ messages, jobs, twins, onNavigateToChat, onNavigateToLogs, onClose }) => {
  const [query, setQuery] = useState('');

  const results = useMemo(() => {
    if (!query.trim() || query.length < 2) return { messages: [], logs: [] };
    
    const lowerQuery = query.toLowerCase();
    
    const filteredMessages = messages.filter(m => 
      m.content.toLowerCase().includes(lowerQuery)
    ).map(m => {
      const twin = twins.find(t => t.id === m.twinId);
      return { ...m, twinName: twin?.name || 'System' };
    });

    const filteredLogs: { job: Job, log: LogEntry }[] = [];
    jobs.forEach(job => {
      job.logs?.forEach(log => {
        if (log.message.toLowerCase().includes(lowerQuery)) {
          filteredLogs.push({ job, log });
        }
      });
    });

    return { messages: filteredMessages, logs: filteredLogs };
  }, [query, messages, jobs, twins]);

  const totalResults = results.messages.length + results.logs.length;

  return (
    <div className="flex-1 flex flex-col bg-[#09090b] overflow-hidden font-display">
      <div className="p-6 border-b border-zinc-800/50 bg-zinc-950/50">
        <div className="max-w-4xl mx-auto">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <span className="material-symbols-outlined text-indigo-500">search</span>
              <h2 className="text-xl font-bold text-white uppercase tracking-tight">Global Tactical Search</h2>
            </div>
            <button 
              onClick={onClose}
              className="p-2 hover:bg-zinc-800 rounded-lg text-zinc-500 transition-colors"
            >
              <span className="material-symbols-outlined">close</span>
            </button>
          </div>
          
          <div className="relative">
            <input 
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Query logs, intelligence reports, or agent directives..."
              className="w-full bg-zinc-900 border border-zinc-800 rounded-2xl px-12 py-4 text-zinc-100 placeholder-zinc-600 outline-none focus:ring-2 focus:ring-indigo-500/30 transition-all shadow-2xl"
            />
            <span className="material-symbols-outlined absolute left-4 top-1/2 -translate-y-1/2 text-zinc-500">terminal</span>
            {query && (
              <div className="absolute right-4 top-1/2 -translate-y-1/2 text-[10px] font-black text-indigo-400 uppercase tracking-widest bg-indigo-500/10 px-2 py-1 rounded">
                {totalResults} Matches
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6 custom-scrollbar">
        <div className="max-w-4xl mx-auto space-y-12">
          {!query ? (
            <div className="flex flex-col items-center justify-center py-20 text-center">
              <div className="w-16 h-16 bg-zinc-900 rounded-full flex items-center justify-center mb-6 border border-zinc-800">
                <span className="material-symbols-outlined text-zinc-700 text-3xl">manage_search</span>
              </div>
              <h3 className="text-zinc-400 font-bold uppercase tracking-widest text-sm mb-2">Neural Index Ready</h3>
              <p className="text-zinc-600 text-xs max-w-xs">Enter at least 2 characters to scan decentralized agent memory and system execution logs.</p>
            </div>
          ) : totalResults === 0 ? (
            <div className="flex flex-col items-center justify-center py-20 text-center">
              <h3 className="text-rose-500 font-bold uppercase tracking-widest text-sm mb-2">No Matches Found</h3>
              <p className="text-zinc-600 text-xs">The query payload yielded zero results across active namespaces.</p>
            </div>
          ) : (
            <>
              {results.messages.length > 0 && (
                <section>
                  <div className="flex items-center gap-2 mb-4">
                    <span className="text-[10px] font-black text-zinc-500 uppercase tracking-[0.2em]">Intel Stream ({results.messages.length})</span>
                    <div className="flex-1 h-px bg-zinc-800"></div>
                  </div>
                  <div className="space-y-3">
                    {results.messages.map(msg => (
                      <button 
                        key={msg.id}
                        onClick={() => msg.twinId && onNavigateToChat(msg.twinId)}
                        className="w-full text-left bg-zinc-900/30 border border-zinc-800 hover:border-indigo-500/50 p-4 rounded-xl transition-all group"
                      >
                        <div className="flex justify-between items-center mb-2">
                          <span className="text-[10px] font-bold text-indigo-400 uppercase tracking-widest">
                            {msg.twinName} <span className="text-zinc-600 mx-1">/</span> {msg.sender}
                          </span>
                          <span className="text-[9px] font-mono text-zinc-600">{msg.timestamp.toLocaleString()}</span>
                        </div>
                        <p className="text-sm text-zinc-300 line-clamp-2 leading-relaxed group-hover:text-white transition-colors">
                          {msg.content}
                        </p>
                      </button>
                    ))}
                  </div>
                </section>
              )}

              {results.logs.length > 0 && (
                <section>
                  <div className="flex items-center gap-2 mb-4">
                    <span className="text-[10px] font-black text-zinc-500 uppercase tracking-[0.2em]">Execution Logs ({results.logs.length})</span>
                    <div className="flex-1 h-px bg-zinc-800"></div>
                  </div>
                  <div className="space-y-3">
                    {results.logs.map(({ job, log }) => (
                      <button 
                        key={log.id}
                        onClick={() => onNavigateToLogs(job.id)}
                        className="w-full text-left bg-[#0d0f14] border border-zinc-800/50 hover:border-cyan-500/50 p-4 rounded-xl transition-all group font-mono"
                      >
                        <div className="flex justify-between items-center mb-2">
                          <div className="flex items-center gap-3">
                            <span className="text-[10px] font-bold text-cyan-400 uppercase tracking-widest">{job.name}</span>
                            <span className={`text-[8px] px-1.5 py-0.5 rounded border uppercase font-bold tracking-tighter shrink-0 ${
                              log.level === 'error' ? 'text-rose-400 border-rose-400/20 bg-rose-400/5' : 
                              log.level === 'warn' ? 'text-amber-400 border-amber-400/20 bg-amber-400/5' : 
                              'text-zinc-500 border-zinc-800 bg-zinc-900'
                            }`}>
                              {log.level}
                            </span>
                          </div>
                          <span className="text-[9px] text-zinc-600">{log.timestamp.toLocaleString()}</span>
                        </div>
                        <p className="text-[11px] text-zinc-400 group-hover:text-zinc-200 transition-colors">
                          {log.message}
                        </p>
                      </button>
                    ))}
                  </div>
                </section>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export default SearchView;