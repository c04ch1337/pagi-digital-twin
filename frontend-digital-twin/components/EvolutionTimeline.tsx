import React, { useState, useEffect } from 'react';
import { getAllRetrospectives, RetrospectiveAnalysis } from '../services/retrospectiveService';
import { getAllPlaybooks, Playbook } from '../services/playbookService';

interface EvolutionTimelineProps {
  className?: string;
}

interface TimelineDataPoint {
  timestamp: Date;
  reliability: number;
  patchApplied?: boolean;
  patchId?: string;
  toolName?: string;
}

const EvolutionTimeline: React.FC<EvolutionTimelineProps> = ({ className = '' }) => {
  const [timelineData, setTimelineData] = useState<TimelineDataPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedPoint, setSelectedPoint] = useState<TimelineDataPoint | null>(null);

  useEffect(() => {
    const fetchData = async () => {
      try {
        setLoading(true);
        const [retrospectivesRes, playbooksRes] = await Promise.all([
          getAllRetrospectives(),
          getAllPlaybooks(),
        ]);

        const retrospectives = retrospectivesRes.retrospectives || [];
        const playbooks = Array.isArray(playbooksRes) ? playbooksRes : (playbooksRes.playbooks || []);

        // Create timeline data points from playbooks and retrospectives
        const dataPoints: TimelineDataPoint[] = [];

        // Add playbook reliability points
        playbooks.forEach((playbook: Playbook) => {
          if (playbook.last_used_at) {
            dataPoints.push({
              timestamp: new Date(playbook.last_used_at),
              reliability: playbook.reliability_score * 100,
              toolName: playbook.tool_name,
            });
          }
        });

        // Add patch markers from retrospectives
        retrospectives.forEach((retro: RetrospectiveAnalysis) => {
          if (retro.suggested_patch) {
            dataPoints.push({
              timestamp: new Date(retro.created_at),
              reliability: (1.0 + retro.reliability_impact) * 100, // Show impact
              patchApplied: false, // Will be true if patch was applied
              patchId: retro.suggested_patch.patch_id,
              toolName: retro.tool_name,
            });
          }
        });

        // Sort by timestamp
        dataPoints.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());

        setTimelineData(dataPoints);
      } catch (err) {
        console.error('[EvolutionTimeline] Failed to fetch data:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 30000); // Refresh every 30 seconds
    return () => clearInterval(interval);
  }, []);

  // Calculate average reliability
  const avgReliability = timelineData.length > 0
    ? timelineData.reduce((sum, point) => sum + point.reliability, 0) / timelineData.length
    : 0;

  // Get min/max for chart scaling
  const minReliability = timelineData.length > 0
    ? Math.min(...timelineData.map(p => p.reliability))
    : 0;
  const maxReliability = timelineData.length > 0
    ? Math.max(...timelineData.map(p => p.reliability))
    : 100;

  const chartHeight = 200;
  const chartWidth = 100; // Percentage
  const padding = 20;

  const getXPosition = (timestamp: Date, index: number) => {
    if (timelineData.length === 0) return 0;
    return ((index / (timelineData.length - 1)) * (100 - padding * 2)) + padding;
  };

  const getYPosition = (reliability: number) => {
    const range = maxReliability - minReliability || 100;
    const normalized = (reliability - minReliability) / range;
    return chartHeight - (normalized * (chartHeight - padding * 2)) - padding;
  };

  if (loading) {
    return (
      <div className={`bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 ${className}`}>
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic text-center py-4">
          Loading evolution timeline...
        </div>
      </div>
    );
  }

  return (
    <div className={`bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 ${className}`}>
      <div className="flex items-center justify-between gap-2 mb-3">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">timeline</span>
          <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
            Evolution Timeline
          </h3>
        </div>
        <div className="text-[10px] text-[var(--text-secondary)] opacity-70">
          Avg: {avgReliability.toFixed(1)}%
        </div>
      </div>

      {timelineData.length === 0 ? (
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic text-center py-4">
          No evolution data yet. Timeline will populate as playbooks are used and patches are applied.
        </div>
      ) : (
        <div className="space-y-3">
          {/* Chart */}
          <div className="relative" style={{ height: `${chartHeight}px` }}>
            <svg
              width="100%"
              height={chartHeight}
              className="absolute inset-0"
              viewBox={`0 0 100 ${chartHeight}`}
              preserveAspectRatio="none"
            >
              {/* Grid lines */}
              {[0, 25, 50, 75, 100].map((percent) => (
                <line
                  key={percent}
                  x1="0"
                  y1={getYPosition(minReliability + (maxReliability - minReliability) * (percent / 100))}
                  x2="100"
                  y2={getYPosition(minReliability + (maxReliability - minReliability) * (percent / 100))}
                  stroke="rgb(var(--bg-steel-rgb))"
                  strokeWidth="0.5"
                  opacity="0.2"
                />
              ))}

              {/* Reliability line */}
              {timelineData.length > 1 && (
                <polyline
                  points={timelineData
                    .map((point, idx) => `${getXPosition(point.timestamp, idx)},${getYPosition(point.reliability)}`)
                    .join(' ')}
                  fill="none"
                  stroke="var(--accent)"
                  strokeWidth="1.5"
                  opacity="0.7"
                />
              )}

              {/* Data points */}
              {timelineData.map((point, idx) => (
                <g key={idx}>
                  {point.patchApplied !== undefined && (
                    <circle
                      cx={getXPosition(point.timestamp, idx)}
                      cy={getYPosition(point.reliability)}
                      r="3"
                      fill={point.patchApplied ? 'var(--success)' : 'rgb(var(--warning-rgb))'}
                      stroke="rgb(var(--surface-rgb))"
                      strokeWidth="1"
                      className="cursor-pointer hover:r-4 transition-all"
                      onClick={() => setSelectedPoint(point)}
                    />
                  )}
                  {!point.patchApplied && (
                    <circle
                      cx={getXPosition(point.timestamp, idx)}
                      cy={getYPosition(point.reliability)}
                      r="2"
                      fill="var(--accent)"
                      className="cursor-pointer hover:r-3 transition-all"
                      onClick={() => setSelectedPoint(point)}
                    />
                  )}
                </g>
              ))}
            </svg>

            {/* Y-axis labels */}
            <div className="absolute left-0 top-0 bottom-0 flex flex-col justify-between text-[8px] text-[var(--text-secondary)] opacity-50">
              <span>{maxReliability.toFixed(0)}%</span>
              <span>{minReliability.toFixed(0)}%</span>
            </div>
          </div>

          {/* Legend */}
          <div className="flex items-center gap-4 text-[9px] text-[var(--text-secondary)]">
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 rounded-full bg-[var(--accent)]"></div>
              <span>Reliability</span>
            </div>
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 rounded-full bg-[var(--success)]"></div>
              <span>Patch Applied</span>
            </div>
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 rounded-full bg-[rgb(var(--warning-rgb))]"></div>
              <span>Patch Available</span>
            </div>
          </div>

          {/* Selected point details */}
          {selectedPoint && (
            <div className="mt-2 p-2 bg-[rgb(var(--bg-secondary-rgb)/0.4)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded text-[9px]">
              <div className="font-semibold text-[var(--text-primary)]">{selectedPoint.toolName}</div>
              <div className="text-[var(--text-secondary)]">
                {selectedPoint.timestamp.toLocaleString()}
              </div>
              <div className="text-[var(--text-secondary)]">
                Reliability: {selectedPoint.reliability.toFixed(1)}%
              </div>
              {selectedPoint.patchId && (
                <div className="text-[var(--accent)]">
                  {selectedPoint.patchApplied ? '✓ Patch Applied' : '⚠ Patch Available'}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default EvolutionTimeline;
