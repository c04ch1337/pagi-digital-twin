import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Search, X, MessageSquare, Brain, BookOpen, Clock, ArrowRight, Bug, ChevronDown, ChevronUp, Check, X as XIcon } from 'lucide-react';
import { performGlobalSearch, SearchResult, submitSearchFeedback } from '../services/phoenixSearchService';

interface PhoenixGlobalSearchProps {
  isOpen: boolean;
  onClose: () => void;
  sessionId: string;
  onNavigateToChat?: (twinId: string) => void;
  onNavigateToMemory?: (namespace: string) => void;
}

const PhoenixGlobalSearch: React.FC<PhoenixGlobalSearchProps> = ({
  isOpen,
  onClose,
  sessionId,
  onNavigateToChat,
  onNavigateToMemory,
}) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [searchStats, setSearchStats] = useState({ chat: 0, memory: 0, playbook: 0 });
  const [debugMode, setDebugMode] = useState(false);
  const [queryTime, setQueryTime] = useState<number | null>(null);
  const [bias, setBias] = useState(0.0); // -1.0 (keyword) to 1.0 (semantic)
  const [biasExpanded, setBiasExpanded] = useState(false);
  const [semanticHighlighting, setSemanticHighlighting] = useState(true);
  const [deepVerify, setDeepVerify] = useState(false); // Cross-Encoder re-ranking
  const [feedbackStates, setFeedbackStates] = useState<Record<string, 'relevant' | 'irrelevant' | null>>({});
  const inputRef = useRef<HTMLInputElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);

  // Focus input when modal opens
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus();
      setQuery('');
      setResults([]);
      setSelectedIndex(0);
      setBias(0.0); // Reset bias when modal opens
      setFeedbackStates({}); // Reset feedback states when modal opens
    }
  }, [isOpen]);

  // Perform search with debounce
  useEffect(() => {
    if (!isOpen || query.trim().length < 2) {
      setResults([]);
      setIsSearching(false);
      return;
    }

    const timeoutId = setTimeout(async () => {
      setIsSearching(true);
      const startTime = performance.now();
      try {
        const response = await performGlobalSearch(query, sessionId, { bias, deepVerify });
        const endTime = performance.now();
        setQueryTime(endTime - startTime);
        setResults(response.results);
        setSearchStats(response.sources);
      } catch (error) {
        console.error('[PhoenixGlobalSearch] Search failed:', error);
        setResults([]);
        setQueryTime(null);
      } finally {
        setIsSearching(false);
      }
    }, 300); // 300ms debounce

    return () => clearTimeout(timeoutId);
  }, [query, sessionId, isOpen, bias, deepVerify]);

  const handleResultClick = useCallback((result: SearchResult) => {
    if (result.type === 'chat' && result.metadata.twinId && onNavigateToChat) {
      onNavigateToChat(result.metadata.twinId);
    } else if (result.type === 'memory' && result.metadata.namespace && onNavigateToMemory) {
      onNavigateToMemory(result.metadata.namespace);
    }
    onClose();
  }, [onNavigateToChat, onNavigateToMemory, onClose]);

  const handleFeedback = useCallback(async (result: SearchResult, isRelevant: boolean) => {
    // Update local state immediately for instant feedback
    setFeedbackStates(prev => ({
      ...prev,
      [result.id]: isRelevant ? 'relevant' : 'irrelevant',
    }));

    // Send feedback to backend
    await submitSearchFeedback(query, result.id, isRelevant);
  }, [query]);

  // Handle keyboard navigation
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(prev => Math.min(prev + 1, results.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(prev => Math.max(prev - 1, 0));
    } else if (e.key === 'Enter' && results[selectedIndex]) {
      e.preventDefault();
      handleResultClick(results[selectedIndex]);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }, [results, selectedIndex, handleResultClick, onClose]);

  // Scroll selected result into view
  useEffect(() => {
    if (resultsRef.current && selectedIndex >= 0) {
      const selectedElement = resultsRef.current.children[selectedIndex] as HTMLElement;
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      }
    }
  }, [selectedIndex, results]);

  const getResultIcon = (type: SearchResult['type']) => {
    switch (type) {
      case 'chat':
        return <MessageSquare className="w-4 h-4" />;
      case 'memory':
        return <Brain className="w-4 h-4" />;
      case 'playbook':
        return <BookOpen className="w-4 h-4" />;
    }
  };

  const formatTimestamp = (timestamp?: string) => {
    if (!timestamp) return '';
    try {
      const date = new Date(timestamp);
      return date.toLocaleDateString() + ' ' + date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } catch {
      return timestamp;
    }
  };

  // Get border color class based on result type
  const getSourceBorderColor = (type: SearchResult['type']) => {
    switch (type) {
      case 'memory':
        return 'border-l-2 border-l-[#FFD700]'; // Gold for Memory
      case 'playbook':
        return 'border-l-2 border-l-[#3B82F6]'; // Blue for Playbook
      case 'chat':
        return 'border-l-2 border-l-[#10B981]'; // Green for Chat
      default:
        return '';
    }
  };

  // Get confidence badge color classes
  const getConfidenceColor = (status?: string) => {
    switch (status) {
      case 'High Confidence':
        return 'text-green-400 bg-green-400/10 border-green-400/20';
      case 'Medium Confidence':
        return 'text-blue-400 bg-blue-400/10 border-blue-400/20';
      case 'Low Confidence':
        return 'text-gray-400 bg-gray-400/10 border-gray-400/20';
      default:
        return 'text-gray-400 bg-gray-400/10 border-gray-400/20';
    }
  };

  // Calculate weight pair from bias
  const weightPair = useMemo(() => {
    const denseWeight = (bias + 1.0) / 2.0; // Maps [-1, 1] to [0, 1]
    const sparseWeight = 1.0 - denseWeight;
    return { denseWeight, sparseWeight };
  }, [bias]);

  // Get bias mode description
  const getBiasMode = () => {
    if (bias < -0.5) return { label: 'Strict Keyword', desc: '100% Sparse / 0% Dense', color: '#10B981' };
    if (bias > 0.5) return { label: 'Strict Semantic', desc: '0% Sparse / 100% Dense', color: '#3B82F6' };
    return { label: 'Balanced', desc: '50% Sparse / 50% Dense', color: '#8B5CF6' };
  };

  // Semantic highlighting: highlight sentences that match query meaning
  const highlightSemanticMatches = (text: string, query: string): string => {
    if (!semanticHighlighting || !query.trim()) return text;
    
    // Simple sentence splitting and keyword-based highlighting
    // In production, this would use actual semantic similarity scoring
    const sentences = text.split(/([.!?]+[\s\n]+)/);
    const queryTerms = query.toLowerCase().split(/\s+/).filter(t => t.length > 2);
    
    return sentences.map(sentence => {
      const sentenceLower = sentence.toLowerCase();
      const matchCount = queryTerms.filter(term => sentenceLower.includes(term)).length;
      const matchRatio = queryTerms.length > 0 ? matchCount / queryTerms.length : 0;
      
      // Highlight sentences with high match ratio
      if (matchRatio >= 0.5 && sentence.trim().length > 10) {
        return `<mark class="bg-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-primary)] px-1 rounded">${sentence}</mark>`;
      }
      return sentence;
    }).join('');
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-[200] flex items-start justify-center pt-[10vh] px-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="w-full max-w-3xl bg-[rgb(var(--surface-rgb)/0.98)] border-2 border-[rgb(var(--bg-steel-rgb)/0.5)] rounded-2xl shadow-2xl overflow-hidden backdrop-blur-md"
        style={{
          boxShadow: '0 0 40px rgba(var(--bg-steel-rgb), 0.3)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search Input */}
        <div className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
          <div className="flex items-center gap-3">
            <Search className="w-5 h-5 text-[var(--bg-steel)]" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Search across chat, memory, and playbooks..."
              className="flex-1 bg-transparent border-none outline-none text-[var(--text-primary)] text-lg placeholder:text-[rgb(var(--text-secondary-rgb)/0.5)]"
            />
            <button
              onClick={() => {
                setDebugMode(!debugMode);
                if (!debugMode) {
                  // Reset bias when enabling debug mode
                  setBias(0.0);
                }
              }}
              className={`p-1.5 hover:bg-[rgb(var(--bg-muted-rgb)/0.5)] rounded transition-colors ${
                debugMode ? 'bg-[rgb(var(--bg-muted-rgb)/0.3)]' : ''
              }`}
              title="Toggle Debug Mode"
            >
              <Bug className={`w-4 h-4 ${debugMode ? 'text-[var(--bg-steel)]' : 'text-[var(--text-secondary)]'}`} />
            </button>
            <button
              onClick={onClose}
              className="p-1 hover:bg-[rgb(var(--bg-muted-rgb)/0.5)] rounded transition-colors"
            >
              <X className="w-5 h-5 text-[var(--text-secondary)]" />
            </button>
          </div>
          
          {/* Search Stats */}
          {(searchStats.chat > 0 || searchStats.memory > 0 || searchStats.playbook > 0) && (
            <div className="flex items-center gap-4 mt-2 text-xs text-[var(--text-secondary)]">
              <span>{searchStats.chat} chat</span>
              <span>{searchStats.memory} memory</span>
              <span>{searchStats.playbook} playbooks</span>
            </div>
          )}
          
          {/* Consensus Bias Slider - Always visible, collapsible */}
          <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
            <button
              onClick={() => setBiasExpanded(!biasExpanded)}
              className="w-full flex items-center justify-between text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
            >
              <div className="flex items-center gap-2">
                <span className="font-medium">Consensus Bias Control</span>
                <span 
                  className="px-2 py-0.5 rounded text-[10px] font-mono"
                  style={{ 
                    backgroundColor: `${getBiasMode().color}20`,
                    color: getBiasMode().color
                  }}
                >
                  {getBiasMode().label}
                </span>
              </div>
              {biasExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
            </button>
            
            {biasExpanded && (
              <div className="mt-3 space-y-3">
                {/* Slider with enhanced gradient */}
                <div className="flex items-center gap-3">
                  <label className="text-xs text-[var(--text-secondary)] whitespace-nowrap min-w-[100px]">
                    Weighting:
                  </label>
                  <div className="flex-1 flex items-center gap-2">
                    <span className="text-[10px] text-[#10B981] font-semibold min-w-[70px]">
                      Keyword
                    </span>
                    <div className="flex-1 relative">
                      <input
                        type="range"
                        min="-1"
                        max="1"
                        step="0.1"
                        value={bias}
                        onChange={(e) => setBias(parseFloat(e.target.value))}
                        className="w-full h-3 bg-transparent rounded-lg appearance-none cursor-pointer"
                        style={{
                          background: `linear-gradient(to right, 
                            #10B981 0%, 
                            #10B981 ${((bias + 1) / 2) * 100}%, 
                            #3B82F6 ${((bias + 1) / 2) * 100}%, 
                            #3B82F6 100%)`,
                          WebkitAppearance: 'none',
                        }}
                      />
                      <div 
                        className="absolute top-0 left-0 h-3 rounded-lg pointer-events-none transition-all duration-200"
                        style={{
                          width: `${((bias + 1) / 2) * 100}%`,
                          background: 'linear-gradient(to right, #10B981, #8B5CF6)',
                          opacity: 0.3,
                        }}
                      />
                    </div>
                    <span className="text-[10px] text-[#3B82F6] font-semibold min-w-[70px] text-right">
                      Semantic
                    </span>
                  </div>
                  <span className="text-xs font-mono text-[var(--bg-steel)] min-w-[50px] text-right font-semibold">
                    {bias.toFixed(1)}
                  </span>
                </div>
                
                {/* Weight visualization */}
                <div className="flex items-center gap-3 text-[10px]">
                  <div className="flex-1 flex items-center gap-2">
                    <div className="flex-1 bg-[rgb(var(--bg-steel-rgb)/0.1)] rounded h-2 relative overflow-hidden">
                      <div 
                        className="absolute left-0 top-0 h-full bg-[#10B981] transition-all duration-200"
                        style={{ width: `${weightPair.sparseWeight * 100}%` }}
                      />
                      <div className="absolute inset-0 flex items-center justify-center text-[8px] font-mono text-white">
                        {weightPair.sparseWeight > 0.05 && `${(weightPair.sparseWeight * 100).toFixed(0)}%`}
                      </div>
                    </div>
                    <span className="text-[#10B981] min-w-[40px]">Sparse</span>
                  </div>
                  <div className="flex-1 flex items-center gap-2">
                    <div className="flex-1 bg-[rgb(var(--bg-steel-rgb)/0.1)] rounded h-2 relative overflow-hidden">
                      <div 
                        className="absolute left-0 top-0 h-full bg-[#3B82F6] transition-all duration-200"
                        style={{ width: `${weightPair.denseWeight * 100}%` }}
                      />
                      <div className="absolute inset-0 flex items-center justify-center text-[8px] font-mono text-white">
                        {weightPair.denseWeight > 0.05 && `${(weightPair.denseWeight * 100).toFixed(0)}%`}
                      </div>
                    </div>
                    <span className="text-[#3B82F6] min-w-[40px]">Dense</span>
                  </div>
                </div>
                
                {/* Mode description */}
                <div className="p-2 bg-[rgb(var(--bg-steel-rgb)/0.1)] rounded text-[10px]">
                  <div className="font-semibold text-[var(--text-primary)] mb-1">
                    {getBiasMode().label}
                  </div>
                  <div className="text-[rgb(var(--text-secondary-rgb)/0.8)]">
                    {getBiasMode().desc}
                  </div>
                  <div className="mt-1.5 text-[rgb(var(--text-secondary-rgb)/0.7)] italic">
                    {bias < -0.5 
                      ? 'Ideal for: Forensic lookups (IPs, GUIDs, exact error codes)'
                      : bias > 0.5
                      ? 'Ideal for: Exploratory research into concepts and relationships'
                      : 'Ideal for: General discovery with both literal matches and context'}
                  </div>
                </div>
                
                {/* Semantic highlighting toggle */}
                <div className="flex items-center justify-between">
                  <label className="text-xs text-[var(--text-secondary)]">
                    Semantic Highlighting
                  </label>
                  <button
                    onClick={() => setSemanticHighlighting(!semanticHighlighting)}
                    className={`relative w-10 h-5 rounded-full transition-colors ${
                      semanticHighlighting ? 'bg-[#3B82F6]' : 'bg-[rgb(var(--bg-steel-rgb)/0.3)]'
                    }`}
                  >
                    <div
                      className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
                        semanticHighlighting ? 'translate-x-5' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
                
                {/* Deep Verify toggle */}
                <div className="flex items-center justify-between">
                  <div className="flex flex-col">
                    <label className="text-xs text-[var(--text-secondary)] font-medium">
                      Deep Verify (Cross-Encoder)
                    </label>
                    <span className="text-[10px] text-[rgb(var(--text-secondary-rgb)/0.7)]">
                      Re-rank top 5 with high-fidelity scoring
                    </span>
                  </div>
                  <button
                    onClick={() => setDeepVerify(!deepVerify)}
                    className={`relative w-10 h-5 rounded-full transition-colors ${
                      deepVerify ? 'bg-[#8B5CF6]' : 'bg-[rgb(var(--bg-steel-rgb)/0.3)]'
                    }`}
                  >
                    <div
                      className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
                        deepVerify ? 'translate-x-5' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Results */}
        <div className="max-h-[60vh] overflow-y-auto" ref={resultsRef}>
          {isSearching ? (
            <div className="p-8 text-center text-[var(--text-secondary)]">
              <div className="inline-block animate-spin rounded-full h-6 w-6 border-b-2 border-[var(--bg-steel)]"></div>
              <p className="mt-2 text-sm">Searching...</p>
            </div>
          ) : results.length === 0 && query.trim().length >= 2 ? (
            <div className="p-8 text-center text-[var(--text-secondary)]">
              <Search className="w-12 h-12 mx-auto mb-2 opacity-50" />
              <p className="text-sm">No results found</p>
            </div>
          ) : results.length === 0 ? (
            <div className="p-8 text-center text-[var(--text-secondary)]">
              <p className="text-sm">Type at least 2 characters to search</p>
              <div className="mt-4 text-xs space-y-1 opacity-70">
                <p>Search across:</p>
                <ul className="list-disc list-inside space-y-0.5">
                  <li>Chat history and conversations</li>
                  <li>P2P memory fragments (semantic search)</li>
                  <li>GitHub playbooks and documentation</li>
                </ul>
              </div>
            </div>
          ) : (
            <div className="divide-y divide-[rgb(var(--bg-steel-rgb)/0.2)]">
              {results.map((result, index) => {
                const feedbackState = feedbackStates[result.id];
                const hasFeedback = feedbackState !== null && feedbackState !== undefined;
                const isRelevant = feedbackState === 'relevant';
                const isIrrelevant = feedbackState === 'irrelevant';
                
                return (
                <div
                  key={result.id}
                  className={`relative transition-all duration-500 ${
                    isRelevant 
                      ? 'ring-2 ring-green-400/60 ring-offset-1 ring-offset-[rgb(var(--surface-rgb)/0.98)] shadow-[0_0_20px_rgba(34,197,94,0.3)]' 
                      : ''
                  } ${
                    isIrrelevant 
                      ? 'ring-2 ring-red-400/60 ring-offset-1 ring-offset-[rgb(var(--surface-rgb)/0.98)] shadow-[0_0_20px_rgba(239,68,68,0.3)]' 
                      : ''
                  }`}
                >
                  <button
                    onClick={() => handleResultClick(result)}
                    className={`w-full text-left p-4 hover:bg-[rgb(var(--bg-muted-rgb)/0.3)] transition-colors ${
                      index === selectedIndex ? 'bg-[rgb(var(--bg-muted-rgb)/0.5)]' : ''
                    } ${getSourceBorderColor(result.type)}`}
                  >
                  <div className="flex items-start gap-3">
                    <div className="mt-0.5 text-[var(--bg-steel)]">
                      {getResultIcon(result.type)}
                    </div>
                    <div className="flex-1 min-w-0">
                      {/* Confidence Badge & Title Row */}
                      <div className="flex items-start justify-between gap-2 mb-1">
                        <div className="flex-1 min-w-0">
                          <h3 className="font-bold text-[var(--text-primary)] truncate">
                            {result.title}
                          </h3>
                        </div>
                        <div className="flex items-center gap-2 flex-shrink-0">
                          {/* Confidence Badge with Tooltip */}
                          {result.metadata.verification_status && (
                            <div className="relative group">
                              <div className={`px-2 py-0.5 rounded border text-[10px] font-mono cursor-help ${getConfidenceColor(result.metadata.verification_status)}`}>
                                {result.metadata.is_promoted && <span className="mr-1 text-amber-400">★</span>}
                                {result.metadata.verification_status.toUpperCase()}
                              </div>
                              {/* Tooltip */}
                              <div className="absolute right-0 top-full mt-1 z-50 w-64 p-2 bg-[rgb(var(--surface-rgb)/0.98)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded shadow-lg opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity duration-200 text-xs backdrop-blur-sm"
                                style={{ 
                                  transform: 'translateX(0)',
                                  maxWidth: 'calc(100vw - 2rem)'
                                }}>
                                <div className="font-semibold text-[var(--text-primary)] mb-1">
                                  Cross-Encoder Verification
                                </div>
                                {result.metadata.crossEncoderScore !== undefined && (
                                  <div className="text-[var(--text-secondary)] mb-1">
                                    <span className="font-mono text-[#8B5CF6]">
                                      {(result.metadata.crossEncoderScore * 100).toFixed(1)}%
                                    </span>
                                    {' '}confidence score
                                  </div>
                                )}
                                <div className="text-[rgb(var(--text-secondary-rgb)/0.8)] text-[10px] mb-1">
                                  This score represents the transformer model's confidence that this document directly answers your query.
                                </div>
                                {result.metadata.is_promoted && (
                                  <div className="text-amber-400 text-[10px] font-semibold mt-1 pt-1 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                                    Ranked #1 due to extreme relevance (&gt;95% match).
                                  </div>
                                )}
                              </div>
                            </div>
                          )}
                          {result.metadata.similarity && (
                            <span className="text-xs text-[var(--text-secondary)]">
                              {(result.metadata.similarity * 100).toFixed(0)}% match
                            </span>
                          )}
                        </div>
                      </div>
                      {/* Optimized Snippet Display - Prioritize verified context */}
                      <p 
                        className="text-sm text-[var(--text-secondary)] mb-2 line-clamp-3"
                        dangerouslySetInnerHTML={{ 
                          __html: semanticHighlighting 
                            ? highlightSemanticMatches(result.snippet || result.preview, query)
                            : (result.snippet || result.preview)
                        }}
                      />
                      {debugMode && (
                        <div className="mb-2 p-2 bg-[rgb(var(--bg-steel-rgb)/0.1)] rounded text-xs">
                          <div className="grid grid-cols-4 gap-2 text-[rgb(var(--text-secondary-rgb)/0.9)]">
                            <div className="flex flex-col">
                              <span className="text-[rgb(var(--text-secondary-rgb)/0.6)] text-[10px]">Dense Score</span>
                              <span className="text-[#3B82F6] font-mono text-[11px]">
                                {result.type === 'memory' && result.metadata.similarity !== undefined
                                  ? result.metadata.similarity.toFixed(4)
                                  : 'N/A'}
                              </span>
                            </div>
                            <div className="flex flex-col">
                              <span className="text-[rgb(var(--text-secondary-rgb)/0.6)] text-[10px]">Sparse Score</span>
                              <span className="text-[#10B981] font-mono text-[11px]">
                                {result.type === 'memory' ? 'N/A' : 'N/A'}
                              </span>
                            </div>
                            <div className="flex flex-col">
                              <span className="text-[rgb(var(--text-secondary-rgb)/0.6)] text-[10px]">RRF Rank</span>
                              <span className="text-[#FFD700] font-mono font-semibold text-[11px]">
                                #{index + 1}
                              </span>
                            </div>
                            <div className="flex flex-col">
                              <span className="text-[rgb(var(--text-secondary-rgb)/0.6)] text-[10px]">Weight</span>
                              <span className="text-[#8B5CF6] font-mono text-[11px]">
                                {result.type === 'memory' 
                                  ? `D:${weightPair.denseWeight.toFixed(2)} S:${weightPair.sparseWeight.toFixed(2)}`
                                  : 'N/A'}
                              </span>
                            </div>
                          </div>
                          {result.metadata.similarity !== undefined && (
                            <div className="mt-1.5 pt-1.5 border-t border-[rgb(var(--bg-steel-rgb)/0.2)] space-y-1">
                              <div className="flex items-center justify-between">
                                <div>
                                  <span className="text-[rgb(var(--text-secondary-rgb)/0.6)] text-[10px]">Final RRF Score: </span>
                                  <span className="text-[var(--bg-steel)] font-mono font-semibold text-[11px]">
                                    {result.metadata.similarity.toFixed(6)}
                                  </span>
                                </div>
                                <div className="text-[10px] text-[rgb(var(--text-secondary-rgb)/0.6)]">
                                  RRF(k=60)
                                </div>
                              </div>
                              {result.metadata.crossEncoderScore !== undefined && (
                                <div className="flex items-center justify-between pt-1 border-t border-[rgb(var(--bg-steel-rgb)/0.15)]">
                                  <div>
                                    <span className="text-[rgb(var(--text-secondary-rgb)/0.6)] text-[10px]">Cross-Encoder Score: </span>
                                    <span className="text-[#8B5CF6] font-mono font-semibold text-[11px]">
                                      {result.metadata.crossEncoderScore.toFixed(6)}
                                    </span>
                                  </div>
                                  <div className="text-[10px] text-[#8B5CF6] font-semibold">
                                    ✓ Deep Verify
                                  </div>
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      )}
                      <div className="flex items-center gap-3 text-xs text-[rgb(var(--text-secondary-rgb)/0.7)]">
                        {result.metadata.timestamp && (
                          <div className="flex items-center gap-1">
                            <Clock className="w-3 h-3" />
                            <span>{formatTimestamp(result.metadata.timestamp)}</span>
                          </div>
                        )}
                        {result.metadata.namespace && (
                          <span className="px-2 py-0.5 bg-[rgb(var(--bg-steel-rgb)/0.2)] rounded">
                            {result.metadata.namespace}
                          </span>
                        )}
                        {result.metadata.filePath && (
                          <span className="truncate font-mono text-[10px]">
                            {result.metadata.filePath}
                          </span>
                        )}
                      </div>
                    </div>
                    <ArrowRight className="w-4 h-4 text-[var(--text-secondary)] opacity-50 flex-shrink-0" />
                  </div>
                  </button>
                  
                  {/* Relevance Feedback Footer */}
                  <div 
                    className="px-4 pb-3 pt-1 border-t border-[rgb(var(--bg-steel-rgb)/0.1)]"
                    onClick={(e) => e.stopPropagation()}
                  >
                    {hasFeedback ? (
                      <div className={`flex items-center gap-2 text-xs transition-all duration-300 ${
                        isRelevant ? 'text-green-400' : 'text-red-400'
                      }`}>
                        <div className={`flex items-center gap-1.5 px-2 py-1 rounded ${
                          isRelevant ? 'bg-green-400/10' : 'bg-red-400/10'
                        }`}>
                          {isRelevant ? (
                            <Check className="w-3 h-3" />
                          ) : (
                            <XIcon className="w-3 h-3" />
                          )}
                          <span className="font-medium">Thank you for your feedback!</span>
                        </div>
                      </div>
                    ) : (
                      <div className="flex items-center gap-2">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleFeedback(result, true);
                          }}
                          className="flex items-center gap-1.5 px-2.5 py-1 text-xs text-[var(--text-secondary)] hover:text-green-400 hover:bg-green-400/10 rounded transition-all duration-200 border border-transparent hover:border-green-400/20"
                          title="Mark as relevant"
                        >
                          <Check className="w-3.5 h-3.5" />
                          <span>Relevant</span>
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleFeedback(result, false);
                          }}
                          className="flex items-center gap-1.5 px-2.5 py-1 text-xs text-[var(--text-secondary)] hover:text-red-400 hover:bg-red-400/10 rounded transition-all duration-200 border border-transparent hover:border-red-400/20"
                          title="Mark as irrelevant"
                        >
                          <XIcon className="w-3.5 h-3.5" />
                          <span>Irrelevant</span>
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              )})}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-3 border-t border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)] text-xs text-[var(--text-secondary)] flex items-center justify-between">
          <div className="flex items-center gap-4">
            <span>↑↓ Navigate</span>
            <span>Enter Select</span>
            <span>Esc Close</span>
            {debugMode && queryTime !== null && (
              <span className="ml-4 px-2 py-0.5 bg-[rgb(var(--bg-steel-rgb)/0.2)] rounded font-mono">
                {queryTime.toFixed(2)}ms
              </span>
            )}
          </div>
          <div className="text-[rgb(var(--text-secondary-rgb)/0.6)]">
            Phoenix Global Search
          </div>
        </div>
      </div>
    </div>
  );
};

export default PhoenixGlobalSearch;
