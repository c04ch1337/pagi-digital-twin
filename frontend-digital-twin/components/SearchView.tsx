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
    <div className="flex-1 flex flex-col bg-[var(--bg-primary)] overflow-hidden font-display text-[var(--text-primary)]">
      <div className="p-6 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <div className="max-w-4xl mx-auto">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">search</span>
              <h2 className="text-xl font-bold text-[var(--text-primary)] uppercase tracking-tight">Global Tactical Search</h2>
            </div>
            <button 
              onClick={onClose}
              className="p-2 hover:bg-[var(--bg-muted)] rounded-lg text-[var(--text-secondary)] transition-colors"
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
              className="w-full bg-[rgb(var(--surface-rgb)/0.35)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-2xl px-12 py-4 text-[var(--text-primary)] placeholder-[rgb(var(--text-secondary-rgb)/0.7)] outline-none focus:ring-2 focus:ring-[rgb(var(--bg-steel-rgb)/0.25)] focus:border-[rgb(var(--bg-steel-rgb)/0.5)] transition-all shadow-2xl"
            />
            <span className="material-symbols-outlined absolute left-4 top-1/2 -translate-y-1/2 text-[var(--bg-steel)]">terminal</span>
            {query && (
              <div className="absolute right-4 top-1/2 -translate-y-1/2 text-[10px] font-black text-[var(--text-primary)] uppercase tracking-widest bg-[rgb(var(--surface-rgb)/0.4)] px-2 py-1 rounded border border-[rgb(var(--bg-steel-rgb)/0.25)]">
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
              <div className="w-16 h-16 bg-[rgb(var(--surface-rgb)/0.35)] rounded-full flex items-center justify-center mb-6 border border-[rgb(var(--bg-steel-rgb)/0.3)]">
                <span className="material-symbols-outlined text-[var(--bg-steel)] text-3xl">manage_search</span>
              </div>
              <h3 className="text-[var(--text-primary)] font-bold uppercase tracking-widest text-sm mb-2">Neural Index Ready</h3>
              <p className="text-[var(--text-secondary)] text-xs max-w-xs">Enter at least 2 characters to scan decentralized agent memory and system execution logs.</p>
            </div>
          ) : totalResults === 0 ? (
            <div className="flex flex-col items-center justify-center py-20 text-center">
              <h3 className="text-[var(--text-secondary)] font-bold uppercase tracking-widest text-sm mb-2">No Matches Found</h3>
              <p className="text-[var(--text-secondary)] text-xs">The query payload yielded zero results across active namespaces.</p>
            </div>
          ) : (
            <>
              {results.messages.length > 0 && (
                <section>
                  <div className="flex items-center gap-2 mb-4">
                    <span className="text-[10px] font-black text-[var(--text-secondary)] uppercase tracking-[0.2em]">Intel Stream ({results.messages.length})</span>
                    <div className="flex-1 h-px bg-[rgb(var(--bg-steel-rgb)/0.25)]"></div>
                  </div>
                  <div className="space-y-3">
                    {results.messages.map(msg => (
                      <button 
                        key={msg.id}
                        onClick={() => msg.twinId && onNavigateToChat(msg.twinId)}
                        className="w-full text-left bg-[rgb(var(--surface-rgb)/0.3)] border border-[rgb(var(--bg-steel-rgb)/0.25)] hover:border-[rgb(var(--bg-steel-rgb)/0.55)] p-4 rounded-xl transition-all group"
                      >
                        <div className="flex justify-between items-center mb-2">
                          <span className="text-[10px] font-bold text-[var(--bg-steel)] uppercase tracking-widest">
                            {msg.twinName} <span className="text-[rgb(var(--text-secondary-rgb)/0.7)] mx-1">/</span> {msg.sender}
                          </span>
                          <span className="text-[9px] font-mono text-[var(--text-secondary)]">{msg.timestamp.toLocaleString()}</span>
                        </div>
                        <p className="text-sm text-[var(--text-primary)] line-clamp-2 leading-relaxed group-hover:text-[var(--text-primary)] transition-colors">
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
                    <span className="text-[10px] font-black text-[var(--text-secondary)] uppercase tracking-[0.2em]">Execution Logs ({results.logs.length})</span>
                    <div className="flex-1 h-px bg-[rgb(var(--bg-steel-rgb)/0.25)]"></div>
                  </div>
                  <div className="space-y-3">
                    {results.logs.map(({ job, log }) => (
                      <button 
                        key={log.id}
                        onClick={() => onNavigateToLogs(job.id)}
                        className="w-full text-left bg-[rgb(var(--surface-rgb)/0.3)] border border-[rgb(var(--bg-steel-rgb)/0.25)] hover:border-[rgb(var(--bg-steel-rgb)/0.55)] p-4 rounded-xl transition-all group font-mono"
                      >
                        <div className="flex justify-between items-center mb-2">
                          <div className="flex items-center gap-3">
                            <span className="text-[10px] font-bold text-[var(--text-primary)] uppercase tracking-widest">{job.name}</span>
                            <span className={`text-[8px] px-1.5 py-0.5 rounded border uppercase font-bold tracking-tighter shrink-0 ${
                              log.level === 'error' ? 'text-[var(--text-secondary)] border-[rgb(var(--text-secondary-rgb)/0.3)] bg-[rgb(var(--text-secondary-rgb)/0.1)]' : 
                              log.level === 'warn' ? 'text-[var(--bg-steel)] border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--bg-steel-rgb)/0.1)]' : 
                              'text-[var(--text-secondary)] border-[rgb(var(--bg-steel-rgb)/0.25)] bg-[rgb(var(--surface-rgb)/0.3)]'
                            }`}>
                              {log.level}
                            </span>
                          </div>
                          <span className="text-[9px] text-[var(--text-secondary)]">{log.timestamp.toLocaleString()}</span>
                        </div>
                        <p className="text-[11px] text-[var(--text-primary)] group-hover:text-[var(--text-primary)] transition-colors">
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
