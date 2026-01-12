import React, { useState, useEffect } from 'react';

interface PromptHistoryEntry {
  id: string;
  timestamp: string;
  previous_prompt: string;
  new_prompt: string;
}

interface EvolutionProps {
  onClose?: () => void;
}

const Evolution: React.FC<EvolutionProps> = ({ onClose }) => {
  const [history, setHistory] = useState<PromptHistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [restoringId, setRestoringId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

  const loadHistory = async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await fetch(`${orchestratorUrl}/v1/prompt/history`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        throw new Error(`Failed to load history: ${response.statusText}`);
      }

      const data = await response.json();
      // Sort by timestamp descending (newest first)
      const sorted = (data.entries || []).sort((a: PromptHistoryEntry, b: PromptHistoryEntry) => 
        new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
      );
      setHistory(sorted);
    } catch (err) {
      console.error('[Evolution] Load error:', err);
      setError(err instanceof Error ? err.message : 'Failed to load prompt history');
    } finally {
      setLoading(false);
    }
  };

  const handleRestore = async (entryId: string) => {
    if (!confirm('Are you sure you want to restore this prompt? This will create a new history entry.')) {
      return;
    }

    setRestoringId(entryId);
    setError(null);

    try {
      const response = await fetch(`${orchestratorUrl}/v1/prompt/restore`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          entry_id: entryId,
        }),
      });

      if (!response.ok) {
        throw new Error(`Failed to restore prompt: ${response.statusText}`);
      }

      const data = await response.json();
      if (!data.success) {
        throw new Error(data.message || 'Restore failed');
      }

      // Reload history after restore
      await loadHistory();
    } catch (err) {
      console.error('[Evolution] Restore error:', err);
      setError(err instanceof Error ? err.message : 'Failed to restore prompt');
    } finally {
      setRestoringId(null);
    }
  };

  useEffect(() => {
    loadHistory();
  }, []);

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  const truncatePrompt = (prompt: string, maxLength: number = 200) => {
    if (prompt.length <= maxLength) return prompt;
    return prompt.substring(0, maxLength) + '...';
  };

  const getDiffPreview = (previous: string, current: string) => {
    // Simple diff: show first 100 chars that differ
    const prevLines = previous.split('\n').slice(0, 5);
    const currLines = current.split('\n').slice(0, 5);
    
    if (prevLines.join('\n') !== currLines.join('\n')) {
      return 'Significant changes detected';
    }
    return 'Minor changes';
  };

  return (
    <div className="flex-1 flex flex-col bg-[#9EC9D9] overflow-hidden font-display text-[#0b1b2b]">
      <div className="p-6 border-b border-[#5381A5]/30 bg-[#90C3EA]">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-[#5381A5]">timeline</span>
            <h2 className="text-xl font-bold text-[#0b1b2b] uppercase tracking-tight">
              Evolutionary Timeline
            </h2>
          </div>
          <div className="flex items-center gap-3">
            <button
              onClick={loadHistory}
              disabled={loading}
              className="px-4 py-2 bg-[#5381A5] text-white rounded hover:bg-[#3d6a8a] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? 'Loading...' : 'Refresh'}
            </button>
            {onClose && (
              <button
                onClick={onClose}
                className="px-4 py-2 bg-[#5381A5] text-white rounded hover:bg-[#3d6a8a] transition-colors"
              >
                Close
              </button>
            )}
          </div>
        </div>

        {error && (
          <div className="mt-4 p-3 bg-red-100 border border-red-400 text-red-700 rounded">
            {error}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-auto p-6">
        {loading && history.length === 0 ? (
          <div className="text-center py-12 text-[#5381A5]">
            <span className="material-symbols-outlined text-4xl mb-2">hourglass_empty</span>
            <p>Loading prompt history...</p>
          </div>
        ) : history.length === 0 ? (
          <div className="text-center py-12 text-[#5381A5]">
            <span className="material-symbols-outlined text-4xl mb-2">history</span>
            <p>No prompt history found. History will be created when prompts are updated.</p>
          </div>
        ) : (
          <div className="max-w-4xl mx-auto">
            {/* Timeline */}
            <div className="relative">
              {/* Vertical line */}
              <div className="absolute left-8 top-0 bottom-0 w-0.5 bg-[#5381A5]/30"></div>

              {history.map((entry, index) => (
                <div key={entry.id} className="relative mb-8 pl-20">
                  {/* Timeline dot */}
                  <div className="absolute left-6 w-4 h-4 bg-[#5381A5] rounded-full border-4 border-[#90C3EA] z-10"></div>

                  {/* Entry card */}
                  <div className="bg-white rounded-lg shadow-sm border border-[#5381A5]/20 p-6 hover:shadow-md transition-shadow">
                    <div className="flex items-start justify-between mb-4">
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-2">
                          <span className="text-sm font-semibold text-[#5381A5]">
                            Version {history.length - index}
                          </span>
                          <span className="text-xs text-[#5381A5]/70">
                            {formatTimestamp(entry.timestamp)}
                          </span>
                        </div>
                        <div className="text-xs text-[#5381A5]/70 mb-3">
                          {getDiffPreview(entry.previous_prompt, entry.new_prompt)}
                        </div>
                      </div>
                      <button
                        onClick={() => handleRestore(entry.id)}
                        disabled={restoringId === entry.id}
                        className="px-4 py-2 bg-emerald-500 text-white rounded text-sm hover:bg-emerald-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        {restoringId === entry.id ? 'Restoring...' : 'Restore'}
                      </button>
                    </div>

                    {/* Prompt preview */}
                    <div className="space-y-4">
                      {expandedId === entry.id ? (
                        <>
                          <div>
                            <div className="text-xs font-semibold text-[#5381A5] mb-2 uppercase tracking-wide">
                              Previous Prompt
                            </div>
                            <div className="bg-[#f0f0f0] p-4 rounded text-sm font-mono text-[#0b1b2b] whitespace-pre-wrap max-h-64 overflow-auto">
                              {entry.previous_prompt}
                            </div>
                          </div>
                          <div>
                            <div className="text-xs font-semibold text-[#5381A5] mb-2 uppercase tracking-wide">
                              New Prompt
                            </div>
                            <div className="bg-[#e8f4f8] p-4 rounded text-sm font-mono text-[#0b1b2b] whitespace-pre-wrap max-h-64 overflow-auto">
                              {entry.new_prompt}
                            </div>
                          </div>
                          <button
                            onClick={() => setExpandedId(null)}
                            className="text-sm text-[#5381A5] hover:text-[#3d6a8a]"
                          >
                            Collapse
                          </button>
                        </>
                      ) : (
                        <>
                          <div>
                            <div className="text-xs font-semibold text-[#5381A5] mb-2 uppercase tracking-wide">
                              Previous Prompt
                            </div>
                            <div className="bg-[#f0f0f0] p-4 rounded text-sm font-mono text-[#0b1b2b]">
                              {truncatePrompt(entry.previous_prompt)}
                            </div>
                          </div>
                          <div>
                            <div className="text-xs font-semibold text-[#5381A5] mb-2 uppercase tracking-wide">
                              New Prompt
                            </div>
                            <div className="bg-[#e8f4f8] p-4 rounded text-sm font-mono text-[#0b1b2b]">
                              {truncatePrompt(entry.new_prompt)}
                            </div>
                          </div>
                          <button
                            onClick={() => setExpandedId(entry.id)}
                            className="text-sm text-[#5381A5] hover:text-[#3d6a8a]"
                          >
                            Expand to view full prompts
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default Evolution;
