/**
 * Qdrant Service
 * Fetches collection statistics and health metrics from Qdrant vector database
 */

export interface HNSWConfig {
  m: number;
  ef_construct: number;
  full_scan_threshold?: number;
}

export interface SegmentInfo {
  id: number;
  type: string;
  status: 'Indexing' | 'Active' | 'Pending Pruning' | 'Optimizing';
  num_vectors?: number;
  num_indexed_vectors?: number;
}

export interface CollectionStats {
  points_count: number;
  indexed_vectors_count: number;
  segments_count: number;
  config: {
    hnsw_config?: HNSWConfig;
  };
  segments?: SegmentInfo[];
}

export interface CollectionInfo {
  result: {
    status: string;
    optimizer_status: string;
    vectors_count: number;
    indexed_vectors_count: number;
    points_count: number;
    segments_count: number;
    config: {
      params: {
        vectors: {
          size: number;
          distance: string;
        };
      };
      hnsw_config?: HNSWConfig;
    };
    payload_schema: Record<string, any>;
  };
}

/**
 * Get Qdrant URL from localStorage or environment variable
 */
const getQdrantUrl = (): string => {
  return (
    localStorage.getItem('root_admin_qdrant_url') ||
    import.meta.env.VITE_QDRANT_URL ||
    'http://127.0.0.1:6334'
  );
};

/**
 * Fetches collection statistics from Qdrant API
 * @param collectionName Collection name (default: 'agent_logs')
 */
export async function fetchCollectionStats(collectionName: string = 'agent_logs'): Promise<CollectionStats> {
  // Try gateway proxy first, then fallback to direct Qdrant
  const gatewayUrl = import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
  const qdrantUrl = getQdrantUrl();
  
  try {
    // Try gateway proxy endpoint first
    let response = await fetch(`${gatewayUrl}/api/memory/stats/${collectionName}`, {
      method: 'GET',
      headers: {
        Accept: 'application/json',
      },
    });

    // If gateway doesn't have the route, try Qdrant directly
    if (!response.ok && response.status === 404) {
      response = await fetch(`${qdrantUrl}/collections/${collectionName}`, {
        method: 'GET',
        headers: {
          Accept: 'application/json',
        },
      });
    }

    if (!response.ok) {
      throw new Error(`Failed to fetch collection stats: ${response.statusText}`);
    }

    const data: CollectionInfo = await response.json();
    
    // Transform Qdrant response to our CollectionStats format
    const stats: CollectionStats = {
      points_count: data.result.points_count || 0,
      indexed_vectors_count: data.result.indexed_vectors_count || 0,
      segments_count: data.result.segments_count || 0,
      config: {
        hnsw_config: data.result.config.hnsw_config,
      },
    };

    // Try to fetch segment details if available
    try {
      const segmentsResponse = await fetch(`${qdrantUrl}/collections/${collectionName}/segments`, {
        method: 'GET',
        headers: {
          Accept: 'application/json',
        },
      });

      if (segmentsResponse.ok) {
        const segmentsData = await segmentsResponse.json();
        if (segmentsData.result && segmentsData.result.segments) {
          stats.segments = segmentsData.result.segments.map((seg: any) => ({
            id: seg.id,
            type: seg.type || 'unknown',
            status: mapSegmentStatus(seg.info?.status || 'unknown'),
            num_vectors: seg.info?.num_vectors || 0,
            num_indexed_vectors: seg.info?.num_indexed_vectors || 0,
          }));
        }
      }
    } catch (segError) {
      console.warn('[QdrantService] Failed to fetch segment details:', segError);
      // Continue without segment details
    }

    return stats;
  } catch (error) {
    console.error('[QdrantService] Error fetching collection stats:', error);
    throw error;
  }
}

/**
 * Maps Qdrant segment status to our status enum
 */
function mapSegmentStatus(status: string): SegmentInfo['status'] {
  const statusLower = status.toLowerCase();
  if (statusLower.includes('indexing') || statusLower.includes('building')) {
    return 'Indexing';
  }
  if (statusLower.includes('pruning') || statusLower.includes('pending')) {
    return 'Pending Pruning';
  }
  if (statusLower.includes('optimizing')) {
    return 'Optimizing';
  }
  return 'Active';
}

/**
 * Calculate Recall Efficiency based on HNSW config and indexed points
 * @param hnswConfig HNSW configuration parameters
 * @param indexedPoints Number of indexed points
 * @param totalPoints Total number of points
 * @returns Recall efficiency percentage (0-100)
 */
export function calculateRecallEfficiency(
  hnswConfig: HNSWConfig | undefined,
  indexedPoints: number,
  totalPoints: number
): number {
  if (!hnswConfig || totalPoints === 0) {
    return 0;
  }

  // Base efficiency from indexed ratio
  const indexedRatio = indexedPoints / totalPoints;
  
  // HNSW parameters influence efficiency
  // Higher m (connections) and ef_construct (search width) improve recall
  const mFactor = Math.min(hnswConfig.m / 16, 1.0); // Normalize m (typical range 4-64)
  const efFactor = Math.min(hnswConfig.ef_construct / 200, 1.0); // Normalize ef_construct (typical range 16-512)
  
  // Combined efficiency score
  const efficiency = indexedRatio * 0.6 + (mFactor * 0.2) + (efFactor * 0.2);
  
  return Math.min(Math.max(efficiency * 100, 0), 100);
}
