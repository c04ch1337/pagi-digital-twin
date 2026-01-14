import React, { useState, useEffect } from 'react';
import { ResponsiveContainer, RadialBarChart, RadialBar, Tooltip, Legend } from 'recharts';
import { fetchCollectionStats, calculateRecallEfficiency, CollectionStats, SegmentInfo } from '../services/qdrantService';
import HoverTooltip from './HoverTooltip';

interface MemoryHealthProps {
  collectionName?: string;
  refreshInterval?: number; // milliseconds
}

const MemoryHealth: React.FC<MemoryHealthProps> = ({ 
  collectionName = 'agent_logs',
  refreshInterval = 5000 
}) => {
  const [stats, setStats] = useState<CollectionStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null);

  const fetchStats = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await fetchCollectionStats(collectionName);
      setStats(data);
      setLastUpdate(new Date());
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch memory health';
      setError(errorMessage);
      console.error('[MemoryHealth] Error:', err);
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
      <div className="p-3 bg-[rgb(var(--surface-rgb)/0.3)] rounded-xl border border-[rgb(var(--bg-steel-rgb)/0.3)]">
        <div className="text-[10px] text-[var(--bg-steel)] text-center py-2">Loading memory health...</div>
      </div>
    );
  }

  if (error && !stats) {
    return (
      <div className="p-3 bg-[rgb(var(--surface-rgb)/0.3)] rounded-xl border border-[rgb(var(--danger-rgb)/0.3)]">
        <div className="text-[10px] text-[rgb(var(--danger-rgb)/0.85)] text-center py-2">
          Error: {error}
        </div>
      </div>
    );
  }

  if (!stats) {
    return null;
  }

  const recallEfficiency = calculateRecallEfficiency(
    stats.config.hnsw_config,
    stats.indexed_vectors_count,
    stats.points_count
  );

  // Prepare data for Recall Efficiency gauge
  const gaugeData = [
    {
      name: 'Recall Efficiency',
      value: recallEfficiency,
      fill:
        recallEfficiency >= 80
          ? 'rgb(var(--bg-steel-rgb))'
          : recallEfficiency >= 60
            ? 'rgb(var(--info-rgb))'
            : 'rgb(var(--bg-muted-rgb))',
    },
  ];

  // Prepare Fragmentation Map data
  const segmentStatusCounts = (stats.segments || []).reduce((acc, seg) => {
    acc[seg.status] = (acc[seg.status] || 0) + 1;
    return acc;
  }, {} as Record<string, number>);

  const fragmentationData = [
    { status: 'Active', count: segmentStatusCounts['Active'] || 0, color: 'rgb(var(--bg-steel-rgb))' },
    { status: 'Indexing', count: segmentStatusCounts['Indexing'] || 0, color: 'rgb(var(--info-rgb))' },
    { status: 'Pending Pruning', count: segmentStatusCounts['Pending Pruning'] || 0, color: 'rgb(var(--bg-muted-rgb))' },
    { status: 'Optimizing', count: segmentStatusCounts['Optimizing'] || 0, color: 'rgb(var(--accent-rgb))' },
  ].filter(item => item.count > 0);

  // Create heatmap grid from segments
  const segments = stats.segments || [];
  const gridSize = Math.ceil(Math.sqrt(segments.length)) || 1;
  const heatmapCells: Array<{ x: number; y: number; status: string; segmentId: number }> = [];

  segments.forEach((seg, idx) => {
    const x = idx % gridSize;
    const y = Math.floor(idx / gridSize);
    heatmapCells.push({
      x,
      y,
      status: seg.status,
      segmentId: seg.id,
    });
  });

  const getStatusColor = (status: string): string => {
    switch (status) {
      case 'Active':
        return 'rgb(var(--bg-steel-rgb))';
      case 'Indexing':
        return 'rgb(var(--info-rgb))';
      case 'Pending Pruning':
        return 'rgb(var(--bg-muted-rgb))';
      case 'Optimizing':
        return 'rgb(var(--accent-rgb))';
      default:
        return 'rgb(var(--info-rgb))';
    }
  };

  const CustomTooltip = ({ active, payload }: any) => {
    if (active && payload && payload.length) {
      return (
        <div className="bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.4)] rounded-lg p-2 shadow-xl">
          <p className="text-[10px] text-[var(--text-secondary)] font-bold mb-1">{payload[0].name}</p>
          <p className="text-xs font-semibold" style={{ color: payload[0].payload.fill }}>
            {payload[0].value.toFixed(1)}%
          </p>
        </div>
      );
    }
    return null;
  };

  return (
    <div className="space-y-4">
      {/* Recall Efficiency Gauge */}
      <HoverTooltip
        title="Recall Efficiency"
        description={`Measures how well the HNSW index is performing. Based on indexed vectors (${stats.indexed_vectors_count}/${stats.points_count}), HNSW config (m=${stats.config.hnsw_config?.m || 'N/A'}, ef_construct=${stats.config.hnsw_config?.ef_construct || 'N/A'}). Higher is better.`}
      >
        <div className="bg-[rgb(var(--surface-rgb)/0.3)] rounded-xl p-3 border border-[rgb(var(--bg-steel-rgb)/0.3)] cursor-help">
          <div className="flex items-center justify-between mb-2">
            <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-widest">
              Recall Efficiency
            </div>
            <div className="text-[10px] text-[var(--bg-steel)] font-mono font-bold">
              {recallEfficiency.toFixed(1)}%
            </div>
          </div>
          <div className="h-24">
            <ResponsiveContainer width="100%" height="100%">
              <RadialBarChart
                cx="50%"
                cy="50%"
                innerRadius="60%"
                outerRadius="90%"
                data={gaugeData}
                startAngle={90}
                endAngle={-270}
              >
                <RadialBar
                  dataKey="value"
                  cornerRadius={4}
                  fill={gaugeData[0].fill}
                />
                <Tooltip content={<CustomTooltip />} />
                <Legend
                  wrapperStyle={{ fontSize: '10px', paddingTop: '5px' }}
                  iconSize={0}
                  content={() => (
                    <div className="text-center">
                      <div className="text-[8px] text-[var(--bg-steel)]">
                        Indexed: {stats.indexed_vectors_count.toLocaleString()} / {stats.points_count.toLocaleString()}
                      </div>
                      {stats.config.hnsw_config && (
                        <div className="text-[8px] text-[var(--bg-muted)] mt-1">
                          m={stats.config.hnsw_config.m}, ef={stats.config.hnsw_config.ef_construct}
                        </div>
                      )}
                    </div>
                  )}
                />
              </RadialBarChart>
            </ResponsiveContainer>
          </div>
        </div>
      </HoverTooltip>

      {/* Fragmentation Map (Heatmap) */}
      <HoverTooltip
        title="Fragmentation Map"
        description={`Visual representation of segment statuses across the collection. Shows ${stats.segments_count} segments: Active (ready), Indexing (building), Pending Pruning (needs cleanup), Optimizing (being optimized).`}
      >
        <div className="bg-[rgb(var(--surface-rgb)/0.3)] rounded-xl p-3 border border-[rgb(var(--bg-steel-rgb)/0.3)] cursor-help">
          <div className="flex items-center justify-between mb-3">
            <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-widest">
              Fragmentation Map
            </div>
            <div className="text-[9px] text-[var(--bg-steel)] font-mono">
              {stats.segments_count} Segments
            </div>
          </div>

          {/* Heatmap Grid */}
          {heatmapCells.length > 0 ? (
            <div className="space-y-2">
              <div
                className="grid gap-1"
                style={{
                  gridTemplateColumns: `repeat(${gridSize}, minmax(0, 1fr))`,
                }}
              >
                {heatmapCells.map((cell, idx) => (
                  <div
                    key={idx}
                    className="aspect-square rounded-sm transition-all hover:scale-110 hover:z-10 relative"
                    style={{
                      backgroundColor: getStatusColor(cell.status),
                      opacity: 0.8,
                    }}
                    title={`Segment ${cell.segmentId}: ${cell.status}`}
                  />
                ))}
              </div>

              {/* Legend */}
              <div className="flex flex-wrap gap-2 justify-center mt-2">
                {fragmentationData.map((item) => (
                  <div key={item.status} className="flex items-center gap-1">
                    <div
                      className="w-3 h-3 rounded-sm"
                      style={{ backgroundColor: item.color }}
                    />
                    <span className="text-[8px] text-[var(--text-secondary)]">
                      {item.status} ({item.count})
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="text-[10px] text-[var(--bg-steel)] text-center py-4">
              No segment data available
            </div>
          )}

          {/* Status Summary */}
          {fragmentationData.length > 0 && (
            <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <div className="grid grid-cols-2 gap-2">
                {fragmentationData.map((item) => (
                  <div key={item.status} className="flex items-center justify-between">
                    <span className="text-[8px] text-[var(--text-secondary)]">{item.status}:</span>
                    <span className="text-[9px] font-bold text-[var(--bg-steel)]">{item.count}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </HoverTooltip>

      {/* Last Update Timestamp */}
      {lastUpdate && (
        <div className="text-[8px] text-[var(--bg-muted)] text-center">
          Updated: {lastUpdate.toLocaleTimeString()}
        </div>
      )}
    </div>
  );
};

export default MemoryHealth;
