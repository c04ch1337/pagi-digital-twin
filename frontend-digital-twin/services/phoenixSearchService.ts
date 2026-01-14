/**
 * Phoenix Global Search Service
 * Unified search across chat history, P2P memory fragments, and GitHub playbooks
 */

export interface SearchResult {
  id: string;
  type: 'chat' | 'memory' | 'playbook';
  title: string;
  content: string;
  preview: string;
  snippet?: string; // Verified context snippet from chunked verifier (best 1000-char excerpt)
  metadata: {
    timestamp?: string;
    twinId?: string;
    namespace?: string;
    similarity?: number;
    filePath?: string;
    crossEncoderScore?: number; // Cross-Encoder score from Deep Verify stage
    verification_status?: 'High Confidence' | 'Medium Confidence' | 'Low Confidence'; // Verification level from Cross-Encoder
    is_promoted?: boolean; // True if result was ranked #1 due to extreme relevance (>95% match)
  };
}

export interface SearchResponse {
  results: SearchResult[];
  total: number;
  sources: {
    chat: number;
    memory: number;
    playbook: number;
  };
}

/**
 * Search chat history from memory service
 */
async function searchChatHistory(query: string, sessionId: string): Promise<SearchResult[]> {
  try {
    const memoryUrl = import.meta.env.VITE_MEMORY_URL || 'http://localhost:8003';
    const response = await fetch(`${memoryUrl}/memory/latest?session_id=${sessionId}`);
    
    if (!response.ok) {
      console.warn('[PhoenixSearch] Failed to fetch chat history:', response.statusText);
      return [];
    }

    const data = await response.json();
    const messages = data.messages || [];
    const lowerQuery = query.toLowerCase();

    return messages
      .filter((msg: any) => {
        const content = (msg.content || msg.text || '').toLowerCase();
        return content.includes(lowerQuery);
      })
      .map((msg: any, idx: number) => ({
        id: `chat-${msg.id || idx}`,
        type: 'chat' as const,
        title: `Chat Message - ${msg.role || 'unknown'}`,
        content: msg.content || msg.text || '',
        preview: truncateText(msg.content || msg.text || '', 150),
        metadata: {
          timestamp: msg.timestamp || new Date().toISOString(),
          twinId: msg.twin_id || 'unknown',
        },
      }))
      .slice(0, 10); // Limit to top 10 chat results
  } catch (error) {
    console.error('[PhoenixSearch] Error searching chat history:', error);
    return [];
  }
}

/**
 * Search memory fragments via Qdrant (semantic search)
 */
async function searchMemoryFragments(
  query: string, 
  namespaces: string[] = ['default', 'corporate_context', 'threat_intel'], 
  bias?: number,
  deepVerify?: boolean
): Promise<SearchResult[]> {
  const results: SearchResult[] = [];
  
  try {
    // Use memory query endpoint from orchestrator
    // The backend searches across all collections (agent_logs, telemetry, quarantine_list)
    const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';
    const requestBody: any = {
      query,
      namespace: namespaces[0], // Optional, backend searches all collections
      top_k: 10,
    };
    
    // Add bias parameter if provided
    if (bias !== undefined) {
      requestBody.bias = bias;
    }
    
    // Add deep_verify parameter if provided
    if (deepVerify !== undefined) {
      requestBody.deep_verify = deepVerify;
    }
    
    const response = await fetch(`${orchestratorUrl}/api/memory/query`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(requestBody),
    }).catch(() => null);

    if (response && response.ok) {
      const data = await response.json();
      const memoryResults = data.results || [];
      
      memoryResults.forEach((result: any, idx: number) => {
        // Determine verification status based on cross-encoder score
        let verification_status: 'High Confidence' | 'Medium Confidence' | 'Low Confidence' | undefined;
        if (result.cross_encoder_score !== undefined) {
          if (result.cross_encoder_score >= 0.85) {
            verification_status = 'High Confidence';
          } else if (result.cross_encoder_score >= 0.65) {
            verification_status = 'Medium Confidence';
          } else {
            verification_status = 'Low Confidence';
          }
        }

        results.push({
          id: `memory-${result.id || `result-${idx}`}`,
          type: 'memory' as const,
          title: `Memory Fragment - ${result.namespace || 'unknown'}`,
          content: result.content || '',
          preview: truncateText(result.content || '', 150),
          snippet: result.snippet, // Verified context snippet from chunked verifier
          metadata: {
            timestamp: result.timestamp,
            namespace: result.namespace,
            similarity: result.similarity || 0,
            twinId: result.twin_id,
            crossEncoderScore: result.cross_encoder_score,
            verification_status,
            is_promoted: result.is_promoted || (result.cross_encoder_score !== undefined && result.cross_encoder_score >= 0.95),
          },
        });
      });
    }
  } catch (error) {
    console.warn('[PhoenixSearch] Memory search not available:', error);
  }

  return results.slice(0, 10); // Limit to top 10 memory results
}

/**
 * Search GitHub playbooks
 */
async function searchPlaybooks(query: string): Promise<SearchResult[]> {
  try {
    // Playbooks are stored in the test-agent-repo
    // We'll need to fetch them via GitHub API or a local file system API
    const playbookRepo = 'c04ch1337/pagi-agent-repo';
    const playbookPath = 'playbooks';
    
    // Try to fetch playbooks via GitHub API or local proxy
    const gatewayUrl = import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
    
    try {
      // Attempt to fetch playbooks list
      // Use playbook search endpoint from orchestrator
      const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';
      const response = await fetch(`${orchestratorUrl}/api/playbooks/search?q=${encodeURIComponent(query)}`, {
        method: 'GET',
        headers: {
          'Accept': 'application/json',
        },
      }).catch(() => null);

      if (response && response.ok) {
        const data = await response.json();
        const playbooks = data.playbooks || [];
        
        return playbooks.map((playbook: any, idx: number) => ({
          id: `playbook-${playbook.id || idx}`,
          type: 'playbook' as const,
          title: playbook.name || 'Playbook',
          content: playbook.content || '',
          preview: truncateText(playbook.snippet || playbook.content || '', 200),
          snippet: playbook.snippet, // Use snippet if available
          metadata: {
            filePath: playbook.path,
            timestamp: playbook.timestamp,
            crossEncoderScore: playbook.cross_encoder_score,
            verification_status: playbook.verification_status,
            is_promoted: playbook.is_promoted,
          },
        }));
      }
    } catch (err) {
      console.debug('[PhoenixSearch] Playbook API not available, using fallback');
    }

    // Fallback: Return empty results if API is not available
    // In a full implementation, this would search local files or use GitHub API directly
    return [];
  } catch (error) {
    console.error('[PhoenixSearch] Error searching playbooks:', error);
    return [];
  }
}

/**
 * Perform unified global search across all sources
 */
export async function performGlobalSearch(
  query: string,
  sessionId: string,
  options?: {
    includeChat?: boolean;
    includeMemory?: boolean;
    includePlaybooks?: boolean;
    bias?: number; // -1.0 (strict keyword) to 1.0 (strict semantic), 0.0 = balanced
    deepVerify?: boolean; // Enable Cross-Encoder re-ranking for top 5 results
  }
): Promise<SearchResponse> {
  if (!query || query.trim().length < 2) {
    return {
      results: [],
      total: 0,
      sources: { chat: 0, memory: 0, playbook: 0 },
    };
  }

  const {
    includeChat = true,
    includeMemory = true,
    includePlaybooks = true,
    bias,
    deepVerify = false,
  } = options || {};

  // Execute searches in parallel
  const [chatResults, memoryResults, playbookResults] = await Promise.all([
    includeChat ? searchChatHistory(query, sessionId) : Promise.resolve([]),
    includeMemory ? searchMemoryFragments(query, ['default', 'corporate_context', 'threat_intel'], bias, deepVerify) : Promise.resolve([]),
    includePlaybooks ? searchPlaybooks(query) : Promise.resolve([]),
  ]);

  // Combine and sort results by relevance
  const allResults = [...chatResults, ...memoryResults, ...playbookResults];
  
  // Sort by relevance (memory similarity, then recency)
  allResults.sort((a, b) => {
    const aScore = (a.metadata.similarity || 0) * 0.5 + (a.metadata.timestamp ? 1 : 0) * 0.5;
    const bScore = (b.metadata.similarity || 0) * 0.5 + (b.metadata.timestamp ? 1 : 0) * 0.5;
    return bScore - aScore;
  });

  return {
    results: allResults,
    total: allResults.length,
    sources: {
      chat: chatResults.length,
      memory: memoryResults.length,
      playbook: playbookResults.length,
    },
  };
}

/**
 * Send relevance feedback for a search result
 */
export async function submitSearchFeedback(
  query: string,
  documentId: string,
  isRelevant: boolean
): Promise<void> {
  try {
    const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';
    const response = await fetch(`${orchestratorUrl}/api/search/feedback`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        query,
        document_id: documentId,
        is_relevant: isRelevant,
      }),
    });

    if (!response.ok) {
      console.warn('[PhoenixSearch] Feedback submission failed:', response.statusText);
    }
  } catch (error) {
    console.error('[PhoenixSearch] Error submitting feedback:', error);
  }
}

/**
 * Truncate text to specified length with ellipsis
 */
function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.substring(0, maxLength - 3) + '...';
}
