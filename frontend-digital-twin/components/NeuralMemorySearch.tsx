import React, { useState } from 'react';
import { Twin } from '../types';

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

  // Mock results for demonstration (will be replaced with API call)
  const mockResults: MemoryResult[] = [
    { 
      id: "mem-001", 
      timestamp: new Date(Date.now() - 3600000).toISOString(), 
      content: "Observed brute-force attempt on SSH port 22; failed to block due to policy.", 
      agent_id: "sentinel", 
      risk_level: "High",
      similarity: 0.92
    },
    { 
      id: "mem-002", 
      timestamp: new Date(Date.now() - 7200000).toISOString(), 
      content: "Configuration backup job completed successfully.", 
      agent_id: "aegis", 
      risk_level: "Low",
      similarity: 0.78
    },
    { 
      id: "mem-003", 
      timestamp: new Date(Date.now() - 10800000).toISOString(), 
      content: "User logged into system 'dev-server-01' using MFA.", 
      agent_id: "trace", 
      risk_level: "Medium",
      similarity: 0.85
    },
    { 
      id: "mem-004", 
      timestamp: new Date(Date.now() - 14400000).toISOString(), 
      content: "Critical vulnerability detected in package manager; patch applied immediately.", 
      agent_id: "sentinel", 
      risk_level: "Critical",
      similarity: 0.95
    },
  ];

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

      // Mock implementation for now
      console.log(`[NeuralMemorySearch] Searching memory for twin '${activeTwin.id}' (${activeTwin.settings.memoryNamespace}) with query: "${query}"`);

      // Simulate API delay
      await new Promise(resolve => setTimeout(resolve, 800));

      // Filter mock results based on query (simple text matching)
      const queryLower = query.toLowerCase();
      const filtered = mockResults.filter(r => 
        r.content.toLowerCase().includes(queryLower) || 
        r.risk_level.toLowerCase() === queryLower ||
        r.agent_id.toLowerCase().includes(queryLower)
      );

      // If no exact matches, return all results (simulating semantic similarity)
      setResults(filtered.length > 0 ? filtered : mockResults.slice(0, 3));
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
      case 'Critical': return 'text-rose-500';
      case 'High': return 'text-orange-500';
      case 'Medium': return 'text-amber-500';
      default: return 'text-zinc-400';
    }
  };

  const getRiskBg = (level: MemoryResult['risk_level']): string => {
    switch (level) {
      case 'Critical': return 'bg-rose-500/10 border-rose-500/30';
      case 'High': return 'bg-orange-500/10 border-orange-500/30';
      case 'Medium': return 'bg-amber-500/10 border-amber-500/30';
      default: return 'bg-white/30 border-[#5381A5]/25';
    }
  };

  return (
    <div className="space-y-3">
      {/* Search Input */}
      <form onSubmit={(e) => { e.preventDefault(); handleSearch(); }} className="relative">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyPress}
          placeholder="Semantic memory query..."
          className="w-full bg-white/30 border border-[#5381A5]/30 rounded-lg pl-8 pr-20 py-2 text-[12px] focus:ring-1 focus:ring-[#5381A5]/30 focus:border-[#5381A5]/60 outline-none transition-all placeholder-[#163247]/70 text-[#0b1b2b]"
          disabled={isLoading}
        />
        <span className="material-symbols-outlined absolute left-2.5 top-1/2 -translate-y-1/2 text-[#5381A5] text-[14px]">
          search
        </span>
        {isLoading && (
          <div className="absolute right-12 top-1/2 -translate-y-1/2 w-3 h-3 border border-[#5381A5] border-t-transparent rounded-full animate-spin"></div>
        )}
        <button
          type="submit"
          disabled={isLoading || !query.trim()}
          className="absolute right-2 top-1/2 -translate-y-1/2 px-2 py-1 bg-[#5381A5] hover:bg-[#437091] disabled:opacity-50 disabled:cursor-not-allowed text-white text-[10px] font-semibold rounded transition-all"
        >
          Search
        </button>
      </form>

      {/* Error Message */}
      {error && (
        <div className="p-2 bg-rose-500/10 border border-rose-500/30 rounded-lg">
          <p className="text-[10px] text-rose-400">{error}</p>
        </div>
      )}

      {/* Results */}
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
