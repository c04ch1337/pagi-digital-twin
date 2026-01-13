import * as React from 'react';
import { X, Play, Trash2, Calendar, User, FileText, Search, Lightbulb } from 'lucide-react';

interface MediaItem {
  filename: string;
  size_bytes: number;
  stored_path: string;
  ts_ms?: number;
  has_transcript?: boolean;
  has_summary?: boolean;
}

interface TranscriptInsights {
  summary: string;
  key_decisions: string[];
  follow_up_tasks: string[];
}

interface MediaGalleryProps {
  onClose: () => void;
}

export default function MediaGallery({ onClose }: MediaGalleryProps) {
  const [recordings, setRecordings] = React.useState<MediaItem[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [selectedVideo, setSelectedVideo] = React.useState<string | null>(null);
  const [selectedVideoFilename, setSelectedVideoFilename] = React.useState<string | null>(null);
  const [deleting, setDeleting] = React.useState<Set<string>>(new Set());
  const [selectedTranscript, setSelectedTranscript] = React.useState<string | null>(null);
  const [selectedTranscriptFilename, setSelectedTranscriptFilename] = React.useState<string | null>(null);
  const [transcriptLoading, setTranscriptLoading] = React.useState<string | null>(null);
  const [selectedInsights, setSelectedInsights] = React.useState<TranscriptInsights | null>(null);
  const [insightsLoading, setInsightsLoading] = React.useState<string | null>(null);
  const [activeTab, setActiveTab] = React.useState<'video' | 'transcript' | 'insights'>('video');
  const [toastMessage, setToastMessage] = React.useState<string | null>(null);
  const [searchQuery, setSearchQuery] = React.useState('');
  const [searchResults, setSearchResults] = React.useState<Set<string>>(new Set());
  const [summaries, setSummaries] = React.useState<Map<string, TranscriptInsights>>(new Map());
  const loadInsightsRef = React.useRef<((filename: string) => Promise<void>) | null>(null);

  const apiBase = import.meta.env.VITE_API_URL || 'http://127.0.0.1:8181';

  const checkForNewSummaries = React.useCallback(async () => {
    try {
      const response = await fetch(`${apiBase}/api/media/list`);
      if (!response.ok) return;
      const data = await response.json();
      const recordingsList = data.recordings || [];
      
      // Check for new summaries
      setRecordings(prev => {
        let hasNewSummary = false;
        let newSummaryFilename = '';
        
        recordingsList.forEach((rec: MediaItem) => {
          if (rec.has_summary) {
            const existing = prev.find(r => r.filename === rec.filename);
            if (existing && !existing.has_summary) {
              // New summary detected!
              hasNewSummary = true;
              newSummaryFilename = rec.filename;
              // Reload insights for this recording
              if (loadInsightsRef.current) {
                setTimeout(() => loadInsightsRef.current!(rec.filename), 0);
              }
            }
          }
        });
        
        if (hasNewSummary) {
          setToastMessage(`New insights available for ${newSummaryFilename}`);
          setTimeout(() => setToastMessage(null), 5000);
        }
        
        // Update recordings with new summary flags
        return prev.map(r => {
          const updatedRec = recordingsList.find((rec: MediaItem) => rec.filename === r.filename);
          return updatedRec ? { ...r, has_summary: updatedRec.has_summary } : r;
        });
      });
    } catch (err) {
      // Silent fail for polling
      console.warn('Failed to check for new summaries:', err);
    }
  }, [apiBase]);

  React.useEffect(() => {
    loadRecordings();
    
    // Poll for new summaries every 5 seconds when gallery is open
    const pollInterval = setInterval(() => {
      checkForNewSummaries();
    }, 5000);
    
    return () => clearInterval(pollInterval);
  }, [checkForNewSummaries]);

  const loadRecordings = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch(`${apiBase}/api/media/list`);
      if (!response.ok) {
        throw new Error(`Failed to load recordings: ${response.statusText}`);
      }
      const data = await response.json();
      const recordingsList = data.recordings || [];
      setRecordings(recordingsList);
      
      // Pre-load summaries for recordings that have transcripts or summaries
      recordingsList.forEach((rec: MediaItem) => {
        if (rec.has_transcript || rec.has_summary) {
          loadInsights(rec.filename);
        }
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load recordings');
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (filename: string) => {
    if (!confirm(`Delete recording "${filename}"?`)) {
      return;
    }

    setDeleting(prev => new Set(prev).add(filename));
    try {
      const response = await fetch(`${apiBase}/api/media/delete/${encodeURIComponent(filename)}`, {
        method: 'DELETE',
      });
      if (!response.ok) {
        throw new Error(`Failed to delete: ${response.statusText}`);
      }
      // Remove from list
      setRecordings(prev => prev.filter(r => r.filename !== filename));
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to delete recording');
    } finally {
      setDeleting(prev => {
        const next = new Set(prev);
        next.delete(filename);
        return next;
      });
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatTimestamp = (ts_ms?: number): string => {
    if (!ts_ms) return 'Unknown date';
    const date = new Date(Number(ts_ms));
    return date.toLocaleString();
  };

  const parseFilename = (filename: string): { twinId: string; timestamp: string } => {
    // Parse `rec_<twin_id>_<ts_ms>.webm`
    const match = filename.match(/^rec_([^_]+)_(\d+)\./);
    if (match) {
      const [, twinId, tsMs] = match;
      const date = new Date(Number(tsMs));
      return {
        twinId,
        timestamp: date.toLocaleString(),
      };
    }
    return { twinId: 'unknown', timestamp: 'Unknown' };
  };

  const getVideoUrl = (filename: string): string => {
    return `${apiBase}/api/media/view/${encodeURIComponent(filename)}`;
  };

  const loadTranscript = async (filename: string) => {
    setTranscriptLoading(filename);
    setSelectedTranscriptFilename(filename);
    if (!selectedVideoFilename) {
      setSelectedVideoFilename(filename);
    }
    try {
      const response = await fetch(`${apiBase}/api/media/transcript/${encodeURIComponent(filename)}`);
      if (!response.ok) {
        throw new Error(`Failed to load transcript: ${response.statusText}`);
      }
      const data = await response.json();
      setSelectedTranscript(data.transcript);
      
      // Also try to load insights if available
      loadInsights(filename);
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to load transcript');
    } finally {
      setTranscriptLoading(null);
    }
  };

  const loadInsights = React.useCallback(async (filename: string) => {
    // Check if we already have it cached
    if (summaries.has(filename)) {
      setSelectedInsights(summaries.get(filename)!);
      return;
    }

    setInsightsLoading(filename);
    try {
      const response = await fetch(`${apiBase}/api/media/summary/${encodeURIComponent(filename)}`);
      if (!response.ok) {
        // Summary might not exist yet, that's okay
        if (response.status === 404) {
          setSelectedInsights(null);
          return;
        }
        throw new Error(`Failed to load summary: ${response.statusText}`);
      }
      const data = await response.json();
      const insights: TranscriptInsights = data.insights;
      setSelectedInsights(insights);
      setSummaries(prev => new Map(prev).set(filename, insights));
    } catch (err) {
      // Non-critical error
      console.warn('Failed to load insights:', err);
      setSelectedInsights(null);
    } finally {
      setInsightsLoading(null);
    }
  }, [apiBase, summaries]);

  // Store loadInsights in ref for use in checkForNewSummaries
  React.useEffect(() => {
    loadInsightsRef.current = loadInsights;
  }, [loadInsights]);

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      setSearchResults(new Set());
      return;
    }

    // Search via Memory Explorer API (semantic search)
    try {
      const memoryUrl = import.meta.env.VITE_MEMORY_URL || 'http://127.0.0.1:8184';
      const response = await fetch(`${memoryUrl}/memory/query?q=${encodeURIComponent(searchQuery)}&namespace=transcripts`);
      if (!response.ok) {
        throw new Error('Search failed');
      }
      const data = await response.json();
      
      // Extract filenames from search results
      const matchingFilenames = new Set<string>();
      if (data.results && Array.isArray(data.results)) {
        data.results.forEach((result: any) => {
          if (result.metadata && result.metadata.filename) {
            matchingFilenames.add(result.metadata.filename);
          }
        });
      }
      
      // Also do a simple text search in loaded transcripts
      recordings.forEach(rec => {
        if (rec.has_transcript && rec.filename.toLowerCase().includes(searchQuery.toLowerCase())) {
          matchingFilenames.add(rec.filename);
        }
      });
      
      setSearchResults(matchingFilenames);
    } catch (err) {
      // Fallback to simple text search in filenames
      const matching = new Set<string>();
      recordings.forEach(rec => {
        if (rec.filename.toLowerCase().includes(searchQuery.toLowerCase())) {
          matching.add(rec.filename);
        }
      });
      setSearchResults(matching);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-[#0b1b2b] text-[#9EC9D9]">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[#5381A5]/30 px-6 py-4 bg-[#163247]">
        <div className="flex-1">
          <h1 className="text-2xl font-semibold text-[#90C3EA]">Neural Archive</h1>
          <p className="text-sm text-[#78A2C2] mt-1">Browse and manage recorded sessions</p>
        </div>
        
        {/* Search Bar */}
        <div className="flex items-center gap-2 mx-4">
          <div className="relative">
            <Search size={16} className="absolute left-3 top-1/2 transform -translate-y-1/2 text-[#78A2C2]" />
            <input
              type="text"
              placeholder="Search transcripts..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
              className="bg-[#0b1b2b] border border-[#5381A5]/30 rounded-lg px-10 py-2 text-sm text-[#9EC9D9] placeholder-[#78A2C2] focus:outline-none focus:border-[#5381A5] w-64"
            />
          </div>
          <button
            onClick={handleSearch}
            className="px-4 py-2 bg-[#5381A5] hover:bg-[#78A2C2] rounded-lg text-sm transition-colors text-white"
          >
            Search
          </button>
        </div>

        <button
          onClick={onClose}
          className="rounded-full p-2 hover:bg-[#5381A5]/30 transition-colors text-[#90C3EA]"
          title="Close gallery"
        >
          <X size={20} />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {loading && (
          <div className="flex items-center justify-center h-64">
            <div className="text-white/60">Loading recordings...</div>
          </div>
        )}

        {error && (
          <div className="bg-white/10 border border-[#5381A5]/30 rounded-lg p-4 mb-4">
            <p className="text-[#90C3EA]">{error}</p>
            <button
              onClick={loadRecordings}
              className="mt-2 text-sm text-[#90C3EA] hover:text-[#78A2C2] underline"
            >
              Retry
            </button>
          </div>
        )}

        {!loading && !error && recordings.length === 0 && (
          <div className="flex flex-col items-center justify-center h-64 text-[#78A2C2]">
            <Calendar size={48} className="mb-4 opacity-50" />
            <p>No recordings found</p>
            <p className="text-sm mt-2">Start recording to create entries in the Neural Archive</p>
          </div>
        )}

        {!loading && !error && recordings.length > 0 && (
          <>
            {/* Toast Notification */}
            {toastMessage && (
              <div className="fixed top-4 right-4 z-70 bg-[#5381A5] text-white px-4 py-3 rounded-lg shadow-lg animate-in slide-in-from-top border border-[#5381A5]/30">
                {toastMessage}
              </div>
            )}

            {/* Video/Transcript/Insights Modal */}
            {(selectedVideo || selectedTranscript !== null) && (
              <div
                className="fixed inset-0 z-60 bg-[#0b1b2b]/90 flex items-center justify-center p-8"
                onClick={() => {
                  setSelectedVideo(null);
                  setSelectedVideoFilename(null);
                  setSelectedTranscript(null);
                  setSelectedTranscriptFilename(null);
                  setSelectedInsights(null);
                  setActiveTab('video');
                }}
              >
                <div className="relative max-w-6xl w-full bg-[#163247] rounded-lg border border-[#5381A5]/30 p-6 max-h-[90vh] flex flex-col" onClick={(e) => e.stopPropagation()}>
                  <div className="flex items-center justify-between mb-4">
                    <h2 className="text-xl font-semibold text-[#90C3EA]">Recording Details</h2>
                    <button
                      onClick={() => {
                        setSelectedVideo(null);
                        setSelectedVideoFilename(null);
                        setSelectedTranscript(null);
                        setSelectedTranscriptFilename(null);
                        setSelectedInsights(null);
                        setActiveTab('video');
                      }}
                      className="text-[#78A2C2] hover:text-[#90C3EA] transition-colors"
                    >
                      <X size={20} />
                    </button>
                  </div>

                  {/* Tabs */}
                  <div className="flex gap-2 mb-4 border-b border-[#5381A5]/30">
                    <button
                      onClick={() => {
                        setActiveTab('video');
                        // Load video if we have a filename
                        if (selectedVideoFilename && !selectedVideo) {
                          setSelectedVideo(selectedVideoFilename);
                        }
                      }}
                      className={`px-4 py-2 text-sm font-medium transition-colors ${
                        activeTab === 'video'
                          ? 'text-[#5381A5] border-b-2 border-[#5381A5]'
                          : 'text-[#78A2C2] hover:text-[#90C3EA]'
                      }`}
                    >
                      Video
                    </button>
                    <button
                      onClick={() => {
                        setActiveTab('transcript');
                        // Load transcript if we have a filename and haven't loaded yet
                        const filename = selectedVideoFilename || selectedTranscriptFilename;
                        if (filename && selectedTranscript === null && transcriptLoading !== filename) {
                          loadTranscript(filename);
                        }
                      }}
                      className={`px-4 py-2 text-sm font-medium transition-colors ${
                        activeTab === 'transcript'
                          ? 'text-[#5381A5] border-b-2 border-[#5381A5]'
                          : 'text-[#78A2C2] hover:text-[#90C3EA]'
                      }`}
                    >
                      Transcript
                    </button>
                    <button
                      onClick={() => {
                        setActiveTab('insights');
                        // Load insights if we have a filename and haven't loaded yet
                        const filename = selectedVideoFilename || selectedTranscriptFilename;
                        if (filename && selectedInsights === null && !insightsLoading) {
                          loadInsights(filename);
                        }
                      }}
                      className={`px-4 py-2 text-sm font-medium transition-colors ${
                        activeTab === 'insights'
                          ? 'text-[#5381A5] border-b-2 border-[#5381A5]'
                          : 'text-[#78A2C2] hover:text-[#90C3EA]'
                      }`}
                    >
                      Insights
                    </button>
                  </div>

                  {/* Content */}
                  <div className="flex-1 overflow-auto">
                    {activeTab === 'video' && (
                      <div className="w-full">
                        <video
                          src={getVideoUrl(selectedVideo || selectedVideoFilename || '')}
                          controls
                          autoPlay
                          className="w-full rounded-lg shadow-2xl"
                        >
                          Your browser does not support the video tag.
                        </video>
                      </div>
                    )}

                    {activeTab === 'transcript' && (
                      <div className="text-[#90C3EA] whitespace-pre-wrap font-mono text-sm leading-relaxed">
                        {transcriptLoading ? (
                          <div className="text-[#78A2C2] text-center py-8">Loading transcript...</div>
                        ) : selectedTranscript ? (
                          selectedTranscript
                        ) : (
                          <div className="text-[#78A2C2] text-center py-8">No transcript available</div>
                        )}
                      </div>
                    )}

                    {activeTab === 'insights' && (
                      <div className="space-y-6">
                        {insightsLoading && (
                          <div className="text-[#78A2C2] text-center py-8">Loading insights...</div>
                        )}
                        
                        {!insightsLoading && selectedInsights && (
                          <>
                            <div>
                              <h3 className="text-lg font-semibold mb-2 text-[#5381A5]">Summary</h3>
                              <p className="text-[#90C3EA] leading-relaxed">{selectedInsights.summary}</p>
                            </div>

                            <div>
                              <h3 className="text-lg font-semibold mb-2 text-[#5381A5]">Key Decisions</h3>
                              <ul className="space-y-2">
                                {selectedInsights.key_decisions.map((decision, idx) => (
                                  <li key={idx} className="flex items-start gap-2 text-[#90C3EA]">
                                    <span className="text-[#5381A5] mt-1">•</span>
                                    <span>{decision}</span>
                                  </li>
                                ))}
                              </ul>
                            </div>

                            <div>
                              <h3 className="text-lg font-semibold mb-2 text-[#5381A5]">Follow-up Tasks</h3>
                              <ul className="space-y-2">
                                {selectedInsights.follow_up_tasks.map((task, idx) => (
                                  <li key={idx} className="flex items-start gap-2 text-[#90C3EA]">
                                    <input type="checkbox" className="mt-1" />
                                    <span>{task}</span>
                                  </li>
                                ))}
                              </ul>
                            </div>
                          </>
                        )}

                        {!insightsLoading && !selectedInsights && (
                          <div className="text-[#78A2C2] text-center py-8">
                            Insights not available yet. Summarization may still be processing.
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}

            {/* Grid */}
            <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {recordings.map((recording) => {
                const { twinId, timestamp } = parseFilename(recording.filename);
                const isDeleting = deleting.has(recording.filename);
                const isSearchMatch = searchResults.size > 0 && searchResults.has(recording.filename);
                const isHighlighted = isSearchMatch;

                return (
                  <div
                    key={recording.filename}
                    className={`group relative bg-[#0b1b2b] border rounded-lg overflow-hidden transition-all ${
                      isHighlighted 
                        ? 'border-[#5381A5] bg-[#5381A5]/20 shadow-lg shadow-[#5381A5]/20' 
                        : 'border-[#5381A5]/30 hover:border-[#5381A5]/50'
                    }`}
                  >
                    {/* Thumbnail/Preview */}
                    <div
                      className="aspect-video bg-[#163247] flex items-center justify-center cursor-pointer"
                      onClick={() => {
                        setSelectedVideo(recording.filename);
                        setSelectedVideoFilename(recording.filename);
                        setActiveTab('video');
                        // Pre-load transcript and insights if available
                        if (recording.has_transcript) {
                          loadTranscript(recording.filename);
                        }
                        if (recording.has_summary) {
                          loadInsights(recording.filename);
                        }
                      }}
                    >
                      <div className="text-center">
                        <Play size={32} className="mx-auto mb-2 text-[#78A2C2] group-hover:text-[#90C3EA] transition-colors" />
                        <span className="text-xs text-[#78A2C2]">Click to play</span>
                      </div>
                    </div>

                    {/* Info */}
                    <div className="p-3">
                      <div className="flex items-start justify-between mb-2">
                        <div className="flex items-center gap-1.5 text-xs text-[#78A2C2]">
                          <User size={12} />
                          <span className="truncate">{twinId}</span>
                        </div>
                        <div className="flex items-center gap-1">
                          {recording.has_transcript && (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                setSelectedVideo(recording.filename);
                                setSelectedVideoFilename(recording.filename);
                                setActiveTab('transcript');
                                loadTranscript(recording.filename);
                              }}
                              disabled={transcriptLoading === recording.filename}
                              className="p-1 hover:bg-[#5381A5]/20 rounded transition-colors disabled:opacity-50"
                              title="Show transcript"
                            >
                              <FileText size={14} className="text-[#5381A5]" />
                            </button>
                          )}
                          {recording.has_summary && (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                setSelectedVideo(recording.filename);
                                setSelectedVideoFilename(recording.filename);
                                setActiveTab('insights');
                                loadInsights(recording.filename);
                              }}
                              disabled={insightsLoading === recording.filename}
                              className="p-1 hover:bg-[#5381A5]/20 rounded transition-colors disabled:opacity-50"
                              title="Show insights"
                            >
                              <Lightbulb size={14} className="text-[#78A2C2]" />
                            </button>
                          )}
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDelete(recording.filename);
                            }}
                            disabled={isDeleting}
                            className="p-1 hover:bg-[#163247]/50 rounded transition-colors disabled:opacity-50"
                            title="Delete recording"
                          >
                            <Trash2 size={14} className="text-[#78A2C2]" />
                          </button>
                        </div>
                      </div>

                      <div className="flex items-center gap-1.5 text-xs text-[#78A2C2] mb-1">
                        <Calendar size={12} />
                        <span className="truncate">{timestamp}</span>
                      </div>

                      <div className="text-xs text-[#78A2C2] mb-2">
                        {formatFileSize(recording.size_bytes)}
                      </div>

                      {/* Summary Preview - First Sentence */}
                      {recording.has_summary && summaries.has(recording.filename) && (
                        <div className="mt-2 p-2 bg-[#0b1b2b] rounded text-xs text-[#90C3EA] line-clamp-2">
                          {(() => {
                            const summary = summaries.get(recording.filename)?.summary || '';
                            // Extract first sentence (ending with . ! or ?)
                            const firstSentenceMatch = summary.match(/^[^.!?]+[.!?]/);
                            const firstSentence = firstSentenceMatch 
                              ? firstSentenceMatch[0].trim() 
                              : summary.split('.')[0].trim() + (summary.includes('.') ? '.' : '');
                            return firstSentence || summary.substring(0, 100) + (summary.length > 100 ? '...' : '');
                          })()}
                        </div>
                      )}
                    </div>

                    {/* Loading overlay for delete */}
                    {isDeleting && (
                      <div className="absolute inset-0 bg-[#0b1b2b]/60 flex items-center justify-center">
                        <div className="text-sm text-[#90C3EA]">Deleting...</div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
