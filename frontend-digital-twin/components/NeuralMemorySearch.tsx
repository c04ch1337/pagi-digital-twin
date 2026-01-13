import React, { useState } from 'react';
import { Twin } from '../types';
import HoverTooltip from './HoverTooltip';

interface MemoryResult {
  id: string;
  timestamp: string;
  content: string;
  agent_id: string;
  risk_level: 'Low' | 'Medium' | 'High' | 'Critical';
  similarity?: number; // Optional similarity score from vector search
}

interface NeuralMemorySearchProps {
  activeTwin: Twin;
}

const NeuralMemorySearch: React.FC<NeuralMemorySearchProps> = ({ activeTwin }) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<MemoryResult[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async () => {
    if (!query.trim()) {
      setResults([]);
      return;
    }

    setIsLoading(true);
    setError(null);
    setResults([]);

    try {
      // TODO: Replace with actual API call when backend is ready
      console.log(`[NeuralMemorySearch] Searching memory for twin '${activeTwin.id}' (${activeTwin.settings.memoryNamespace}) with query: "${query}"`);

      // TODO: Implement actual API call
      // const apiUrl = import.meta.env.VITE_MEMORY_API_URL || 'http://127.0.0.1:8181/v1/memory/query';
      // const response = await fetch(apiUrl, {
      //   method: 'POST',
      //   headers: { 'Content-Type': 'application/json' },
      //   body: JSON.stringify({ 
      //     query: query.trim(), 
      //     twin_id: activeTwin.id,
      //     namespace: activeTwin.settings.memoryNamespace,
      //     top_k: 10
      //   }),
      // });
      // 
      // if (!response.ok) {
      //   throw new Error(`Memory search failed: ${response.statusText}`);
      // }
      // 
      // const data = await response.json();
      // setResults(data.results || []);

      // No mock data - return empty results until API is implemented
      setResults([]);
    } catch (err) {
      console.error('[NeuralMemorySearch] Search error:', err);
      setError(err instanceof Error ? err.message : 'Failed to search memory');
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSearch();
    }
  };

  // Helper to map risk levels to colors
  const getRiskColor = (level: MemoryResult['risk_level']): string => {
    switch (level) {
      case 'Critical': return 'text-[#163247]';
      case 'High': return 'text-[#5381A5]';
      case 'Medium': return 'text-[#78A2C2]';
      default: return 'text-[#163247]';
    }
  };

  const getRiskBg = (level: MemoryResult['risk_level']): string => {
    switch (level) {
      case 'Critical': return 'bg-[#163247]/10 border-[#163247]/30';
      case 'High': return 'bg-[#5381A5]/10 border-[#5381A5]/30';
      case 'Medium': return 'bg-[#78A2C2]/10 border-[#78A2C2]/30';
      default: return 'bg-white/30 border-[#5381A5]/25';
    }
  };

  return (
    <div className="space-y-3">
      {/* Search Input */}
      <form onSubmit={(e) => { e.preventDefault(); handleSearch(); }} className="relative">
        <HoverTooltip
          title="Semantic Query"
          description={`Search the Vector Vault for meaning-based matches inside the active namespace: ${activeTwin.settings.memoryNamespace}. Press Enter to run.`}
        >
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyPress}
            placeholder="Semantic memory query..."
            className="w-full bg-white/30 border border-[#5381A5]/30 rounded-lg pl-8 pr-20 py-2 text-[12px] focus:ring-1 focus:ring-[#5381A5]/30 focus:border-[#5381A5]/60 outline-none transition-all placeholder-[#163247]/70 text-[#0b1b2b]"
            disabled={isLoading}
          />
        </HoverTooltip>

        <span className="material-symbols-outlined absolute left-2.5 top-1/2 -translate-y-1/2 text-[#5381A5] text-[14px]">
          search
        </span>
        {isLoading && (
          <div className="absolute right-12 top-1/2 -translate-y-1/2 w-3 h-3 border border-[#5381A5] border-t-transparent rounded-full animate-spin"></div>
        )}

        <HoverTooltip
          title="Run Search"
          description="Execute the semantic query against the active namespace and return the closest matching memory shards."
        >
          <button
            type="submit"
            disabled={isLoading || !query.trim()}
            className="absolute right-2 top-1/2 -translate-y-1/2 px-2 py-1 bg-[#5381A5] hover:bg-[#437091] disabled:opacity-50 disabled:cursor-not-allowed text-white text-[10px] font-semibold rounded transition-all"
          >
            Search
          </button>
        </HoverTooltip>
      </form>

      {/* Error Message */}
      {error && (
        <div className="p-2 bg-rose-500/10 border border-rose-500/30 rounded-lg">
          <p className="text-[10px] text-rose-400">{error}</p>
        </div>
      )}

      {/* Results */}
      <HoverTooltip
        title="Results"
        description="Memory hits returned from the semantic query. Risk level is a UI classification; similarity indicates approximate match confidence."
      >
        <div className="space-y-2 max-h-64 overflow-y-auto custom-scrollbar">
        {results.length === 0 && !isLoading && query ? (
          <div className="text-center py-4">
            <p className="text-[10px] text-[#163247] italic">No semantic matches found.</p>
          </div>
        ) : results.length === 0 && !isLoading && !query ? (
          <div className="text-center py-4">
            <p className="text-[10px] text-[#163247] italic">Enter a query to search the Neural Archive.</p>
            <p className="text-[9px] text-[#163247]/70 mt-1">Namespace: {activeTwin.settings.memoryNamespace}</p>
          </div>
        ) : (
          results.map((result) => (
            <div
              key={result.id}
              className={`p-2.5 rounded-lg border transition-all hover:border-[#5381A5]/55 ${getRiskBg(result.risk_level)}`}
            >
              {/* Header with Risk Level and Timestamp */}
              <div className="flex items-center justify-between mb-1.5">
                <div className="flex items-center gap-2">
                  <span className={`text-[8px] font-bold uppercase tracking-widest ${getRiskColor(result.risk_level)}`}>
                    {result.risk_level}
                  </span>
                  {result.similarity && (
                    <span className="text-[9px] text-[#163247] font-mono">
                      {(result.similarity * 100).toFixed(0)}% match
                    </span>
                  )}
                </div>
                <span className="text-[9px] text-[#163247] font-mono">
                  {new Date(result.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                </span>
              </div>

              {/* Content */}
              <p className="text-[11px] text-[#0b1b2b] leading-snug mb-1.5 line-clamp-3">
                {result.content}
              </p>

              {/* Footer with Agent ID */}
              <div className="flex items-center justify-between">
                <span className="text-[9px] font-semibold text-[#5381A5] uppercase tracking-wider">
                  {result.agent_id}
                </span>
                <span className="text-[9px] text-[#163247] font-mono">
                  {result.id}
                </span>
              </div>
            </div>
          ))
        )}
        </div>
      </HoverTooltip>

      {/* Results Count */}
      {results.length > 0 && (
        <div className="text-center pt-2 border-t border-[#5381A5]/20">
          <p className="text-[10px] text-[#163247]">
            Found {results.length} {results.length === 1 ? 'result' : 'results'}
          </p>
        </div>
      )}
    </div>
  );
};

export default NeuralMemorySearch;
