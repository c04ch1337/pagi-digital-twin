import React, { useState, useEffect } from 'react';
import { fetchCollectionStats, CollectionStats, SegmentInfo } from '../services/qdrantService';
import HoverTooltip from './HoverTooltip';

interface FragmentationHeatmapProps {
  collectionName?: string;
  refreshInterval?: number; // milliseconds
  onSegmentClick?: (segmentId: number) => void;
}

const FragmentationHeatmap: React.FC<FragmentationHeatmapProps> = ({ 
  collectionName = 'agent_logs',
  refreshInterval = 5000,
  onSegmentClick
}) => {
  const [stats, setStats] = useState<CollectionStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [hoveredSegment, setHoveredSegment] = useState<SegmentInfo | null>(null);
  const [optimizingSegments, setOptimizingSegments] = useState<Set<number>>(new Set());

  const fetchStats = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await fetchCollectionStats(collectionName);
      setStats(data);
      
      // Track optimizing segments for flashing effect
      const optimizing = new Set<number>();
      (data.segments || []).forEach(seg => {
        if (seg.status === 'Optimizing') {
          optimizing.add(seg.id);
        }
      });
      setOptimizingSegments(optimizing);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch fragmentation data';
      setError(errorMessage);
      console.error('[FragmentationHeatmap] Error:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchStats();
    const interval = setInterval(fetchStats, refreshInterval);
    return () => clearInterval(interval);
  }, [collectionName, refreshInterval]);

  if (loading && !stats) {
    return (
      <div className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 shadow-lg">
        <div className="text-center py-8">
          <div className="text-sm text-[var(--text-secondary)] mb-2">Loading fragmentation heatmap...</div>
          <div className="w-8 h-8 border-4 border-[var(--bg-steel)] border-t-transparent rounded-full animate-spin mx-auto"></div>
        </div>
      </div>
    );
  }

  if (error && !stats) {
    return (
      <div className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--danger-rgb)/0.3)] rounded-xl p-4 shadow-lg">
        <div className="text-center py-4">
          <div className="text-sm text-[rgb(var(--danger-rgb)/0.85)] mb-2">Error loading heatmap</div>
          <div className="text-xs text-[var(--text-secondary)] mb-4">{error}</div>
          <button
            onClick={fetchStats}
            className="px-4 py-2 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (!stats || !stats.segments || stats.segments.length === 0) {
    return (
      <div className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 shadow-lg">
        <div className="text-center py-8">
          <div className="text-sm text-[var(--text-secondary)]">No segment data available</div>
        </div>
      </div>
    );
  }

  const segments = stats.segments;
  const totalVectors = stats.points_count;
  const gridSize = Math.ceil(Math.sqrt(segments.length)) || 1;

  // Calculate density for each segment
  const maxDensity = Math.max(...segments.map(s => (s.num_vectors || 0)), 1);
  
  const getDensityColor = (segment: SegmentInfo): string => {
    const density = (segment.num_vectors || 0) / maxDensity;
    const baseOpacity = 0.7;
    
    // Color based on status
    if (segment.status === 'Optimizing') {
      return `rgb(var(--accent-rgb) / ${baseOpacity + 0.2})`;
    }
    if (segment.status === 'Indexing') {
      return `rgb(var(--info-rgb) / ${baseOpacity + density * 0.3})`;
    }
    if (segment.status === 'Pending Pruning') {
      return `rgb(var(--bg-muted-rgb) / ${baseOpacity + density * 0.3})`;
    }
    // Active segments - color intensity based on density
    return `rgb(var(--bg-steel-rgb) / ${baseOpacity + density * 0.3})`;
  };

  const getIndexType = (segment: SegmentInfo): string => {
    // Determine index type based on segment properties
    if (segment.num_indexed_vectors && segment.num_indexed_vectors > 0) {
      return 'HNSW';
    }
    return 'Plain';
  };

  return (
    <div className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6 shadow-lg">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">heat_map</span>
          <h3 className="text-sm font-bold text-[var(--text-secondary)] uppercase tracking-wider">
            Fragmentation Heatmap
          </h3>
        </div>
        <div className="text-xs text-[var(--bg-steel)] font-mono">
          {stats.segments_count} Segments • {totalVectors.toLocaleString()} Vectors
        </div>
      </div>

      {/* Heatmap Grid */}
      <div className="mb-4">
        <div className="grid gap-1" style={{ gridTemplateColumns: `repeat(${gridSize}, minmax(0, 1fr))` }}>
          {segments.map((segment, idx) => {
            const isOptimizing = optimizingSegments.has(segment.id);
            const indexType = getIndexType(segment);
            const density = (segment.num_vectors || 0) / maxDensity;
            
            return (
              <HoverTooltip
                key={segment.id}
                title={`Segment ${segment.id}`}
                description={
                  <div className="text-xs space-y-1">
                    <div><strong>Status:</strong> {segment.status}</div>
                    <div><strong>Points:</strong> {segment.num_vectors?.toLocaleString() || 0}</div>
                    <div><strong>Index Type:</strong> {indexType}</div>
                    <div><strong>Indexed Vectors:</strong> {segment.num_indexed_vectors?.toLocaleString() || 0}</div>
                    <div><strong>Density:</strong> {(density * 100).toFixed(1)}%</div>
                  </div>
                }
              >
                <div
                  className="aspect-square rounded-sm transition-all hover:scale-110 hover:z-10 relative cursor-pointer border border-[rgb(var(--bg-steel-rgb)/0.2)]"
                  style={{
                    backgroundColor: getDensityColor(segment),
                    animation: isOptimizing ? 'pulse 2s ease-in-out infinite' : undefined,
                    boxShadow: hoveredSegment?.id === segment.id 
                      ? '0 0 12px rgb(var(--bg-steel-rgb) / 0.55)'
                      : '0 2px 4px rgb(var(--overlay-rgb) / 0.12)',
                  }}
                  onMouseEnter={() => setHoveredSegment(segment)}
                  onMouseLeave={() => setHoveredSegment(null)}
                  onClick={() => onSegmentClick?.(segment.id)}
                  title={`Segment ${segment.id}: ${segment.status} (${segment.num_vectors || 0} vectors)`}
                >
                  {/* Density indicator */}
                  <div 
                    className="absolute inset-0 rounded-sm"
                    style={{
                      background: `linear-gradient(to top, rgb(var(--overlay-rgb) / ${density * 0.2}), transparent)`,
                    }}
                  />
                  {/* Optimization indicator */}
                  {isOptimizing && (
                    <div className="absolute top-1 right-1 w-2 h-2 bg-[rgb(var(--warning-rgb)/0.95)] rounded-full animate-ping" />
                  )}
                </div>
              </HoverTooltip>
            );
          })}
        </div>
      </div>

      {/* Legend */}
      <div className="mb-4 pt-4 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
        <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-wider mb-2">
          Segment Status
        </div>
        <div className="flex flex-wrap gap-3">
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 rounded-sm bg-[var(--bg-steel)] opacity-80" />
            <span className="text-xs text-[var(--text-secondary)]">Active</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 rounded-sm bg-[var(--bg-muted)] opacity-80" />
            <span className="text-xs text-[var(--text-secondary)]">Indexing</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 rounded-sm bg-[var(--bg-secondary)] opacity-80 animate-pulse" />
            <span className="text-xs text-[var(--text-secondary)]">Optimizing</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 rounded-sm bg-[var(--text-secondary)] opacity-80" />
            <span className="text-xs text-[var(--text-secondary)]">Pending Pruning</span>
          </div>
        </div>
      </div>

      {/* Axes Labels */}
      <div className="flex items-center justify-between text-[9px] text-[var(--bg-steel)] font-bold uppercase tracking-wider pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
        <div className="flex items-center gap-1">
          <span className="material-symbols-outlined text-xs">database</span>
          <span>Knowledge Density</span>
        </div>
        <div className="flex items-center gap-1">
          <span className="material-symbols-outlined text-xs">speed</span>
          <span>Retrieval Speed</span>
        </div>
      </div>

      {/* Status Summary */}
      <div className="mt-4 pt-4 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
        <div className="grid grid-cols-2 gap-3 text-xs">
          <div>
            <div className="text-[var(--bg-steel)] font-bold mb-1">Total Segments</div>
            <div className="text-[var(--text-secondary)] font-mono">{stats.segments_count}</div>
          </div>
          <div>
            <div className="text-[var(--bg-steel)] font-bold mb-1">Total Vectors</div>
            <div className="text-[var(--text-secondary)] font-mono">{totalVectors.toLocaleString()}</div>
          </div>
          <div>
            <div className="text-[var(--bg-steel)] font-bold mb-1">Indexed Vectors</div>
            <div className="text-[var(--text-secondary)] font-mono">{stats.indexed_vectors_count.toLocaleString()}</div>
          </div>
          <div>
            <div className="text-[var(--bg-steel)] font-bold mb-1">Optimizing</div>
            <div className="text-[var(--text-secondary)] font-mono">{optimizingSegments.size}</div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default FragmentationHeatmap;
