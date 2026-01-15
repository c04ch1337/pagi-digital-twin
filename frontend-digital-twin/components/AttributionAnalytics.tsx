import React, { useEffect } from 'react';
import { useDomainAttribution } from '../context/DomainAttributionContext';
import HoverTooltip from './HoverTooltip';

interface AttributionAnalyticsProps {
  className?: string;
}

const AttributionAnalytics: React.FC<AttributionAnalyticsProps> = ({ className = '' }) => {
  const { sessionAverage, getDomainDrift, knowledgeBaseStats, incrementKnowledgeBase } = useDomainAttribution();
  const domainDrift = getDomainDrift();

  // Listen for ingestion-complete events to update knowledge base stats
  useEffect(() => {
    const handleIngestionComplete = (event: CustomEvent<{ domain: string; fileName: string }>) => {
      const { domain } = event.detail;
      // The IngestorDashboard already calls incrementKnowledgeBase, but we can add
      // additional logic here if needed (e.g., re-fetching vector counts from Qdrant)
      console.log(`[AttributionAnalytics] Ingestion complete: ${event.detail.fileName} → ${domain}`);
      
      // Check for domain drift after knowledge base update
      // Domain drift occurs when one domain becomes disproportionately large
      setTimeout(() => {
        const drift = getDomainDrift();
        if (drift === 'technical' && domain === 'Mind') {
          console.warn('[AttributionAnalytics] Domain drift detected: Technical focus (Mind domain dominance)');
        } else if (drift === 'ethical' && domain === 'Soul') {
          console.warn('[AttributionAnalytics] Domain drift detected: Ethics-driven (Soul domain dominance)');
        }
      }, 100);
      
      // TODO: Optionally trigger a re-fetch of total_vector_count from Qdrant
      // This would require adding a service method to fetch vector counts per domain
    };

    window.addEventListener('ingestion-complete', handleIngestionComplete as EventListener);
    
    return () => {
      window.removeEventListener('ingestion-complete', handleIngestionComplete as EventListener);
    };
  }, [incrementKnowledgeBase, getDomainDrift]);

  if (!sessionAverage || sessionAverage.messageCount === 0) {
    return (
      <div className={`bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 ${className}`}>
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic text-center py-4">
          No attribution data available. Start a conversation to see analytics.
        </div>
      </div>
    );
  }

  // Radar chart dimensions
  const size = 200;
  const center = size / 2;
  const radius = 80;
  const domains = [
    { name: 'Mind', value: sessionAverage.mind, angle: -Math.PI / 2, color: 'var(--bg-steel)' },
    { name: 'Body', value: sessionAverage.body, angle: 0, color: 'var(--bg-steel)' },
    { name: 'Heart', value: sessionAverage.heart, angle: Math.PI / 2, color: 'rgb(var(--warning-rgb))' },
    { name: 'Soul', value: sessionAverage.soul, angle: Math.PI, color: 'rgb(var(--danger-rgb))' },
  ];

  // Calculate polygon points for radar chart
  const getPoint = (angle: number, value: number) => {
    const normalizedValue = value / 100; // Normalize to 0-1
    const r = radius * normalizedValue;
    const x = center + r * Math.cos(angle);
    const y = center + r * Math.sin(angle);
    return { x, y };
  };

  const points = domains.map(d => getPoint(d.angle, d.value));
  const pathData = points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ') + ' Z';

  // Grid circles for reference
  const gridLevels = [0.25, 0.5, 0.75, 1.0];

  const getDriftColor = () => {
    switch (domainDrift) {
      case 'technical': return 'text-[var(--bg-steel)]';
      case 'reactive': return 'text-[var(--bg-steel)]';
      case 'personal': return 'text-[rgb(var(--warning-rgb))]';
      case 'ethical': return 'text-[rgb(var(--danger-rgb))]';
      default: return 'text-[var(--text-secondary)]';
    }
  };

  const getDriftLabel = () => {
    switch (domainDrift) {
      case 'technical': return 'Technical Focus';
      case 'reactive': return 'Reactive Mode';
      case 'personal': return 'Personalized';
      case 'ethical': return 'Ethics-Driven';
      default: return 'Balanced';
    }
  };

  return (
    <div className={`bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 ${className}`}>
      <div className="flex items-center justify-between mb-4">
        <HoverTooltip
          title="Attribution Analytics"
          description="Radar chart showing the balance of knowledge domains across the current session. The shape indicates which domains are driving the cluster's reasoning."
        >
          <div className="flex items-center gap-2 cursor-help">
            <span className="material-symbols-outlined text-[14px] text-[var(--bg-steel)]">insights</span>
            <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
              Session Balance
            </h3>
          </div>
        </HoverTooltip>
        <div className={`text-[9px] font-bold uppercase ${getDriftColor()}`}>
          {getDriftLabel()}
        </div>
      </div>

      <div className="flex flex-col items-center">
        {/* Radar Chart */}
        <svg width={size} height={size} className="mb-4">
          {/* Grid circles */}
          {gridLevels.map((level, i) => (
            <circle
              key={i}
              cx={center}
              cy={center}
              r={radius * level}
              fill="none"
              stroke="rgb(var(--bg-steel-rgb)/0.2)"
              strokeWidth="1"
            />
          ))}

          {/* Grid lines (axes) */}
          {domains.map((domain, i) => {
            const point = getPoint(domain.angle, 100);
            return (
              <line
                key={i}
                x1={center}
                y1={center}
                x2={point.x}
                y2={point.y}
                stroke="rgb(var(--bg-steel-rgb)/0.2)"
                strokeWidth="1"
              />
            );
          })}

          {/* Data polygon */}
          <path
            d={pathData}
            fill="rgb(var(--bg-steel-rgb)/0.2)"
            stroke="var(--bg-steel)"
            strokeWidth="2"
            opacity="0.6"
          />

          {/* Domain labels */}
          {domains.map((domain, i) => {
            const labelPoint = getPoint(domain.angle, 110);
            return (
              <g key={i}>
                <text
                  x={labelPoint.x}
                  y={labelPoint.y}
                  textAnchor="middle"
                  dominantBaseline="middle"
                  className="text-[10px] font-bold fill-[var(--text-primary)]"
                >
                  {domain.name}
                </text>
                <text
                  x={labelPoint.x}
                  y={labelPoint.y + 12}
                  textAnchor="middle"
                  dominantBaseline="middle"
                  className="text-[9px] fill-[var(--text-secondary)]"
                >
                  {Math.round(domain.value)}%
                </text>
              </g>
            );
          })}
        </svg>

        {/* Statistics */}
        <div className="w-full space-y-2">
          <div className="grid grid-cols-2 gap-2 text-[9px]">
            <div className="bg-[rgb(var(--surface-rgb)/0.4)] p-2 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <div className="text-[var(--text-secondary)] mb-1">Messages</div>
              <div className="text-[var(--bg-steel)] font-bold">{sessionAverage.messageCount}</div>
            </div>
            <div className="bg-[rgb(var(--surface-rgb)/0.4)] p-2 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <div className="text-[var(--text-secondary)] mb-1">Drift</div>
              <div className={`font-bold ${getDriftColor()}`}>{getDriftLabel()}</div>
            </div>
          </div>

          {/* Domain breakdown */}
          <div className="space-y-1.5">
            {domains.map((domain, i) => (
              <div key={i} className="flex items-center justify-between text-[9px]">
                <div className="flex items-center gap-2">
                  <div 
                    className="w-2 h-2 rounded-full" 
                    style={{ backgroundColor: domain.color }}
                  />
                  <span className="text-[var(--text-secondary)]">{domain.name}</span>
                </div>
                <span className="text-[var(--bg-steel)] font-mono font-bold">
                  {Math.round(domain.value)}%
                </span>
              </div>
            ))}
          </div>

          {/* Knowledge Base Growth */}
          {knowledgeBaseStats && (knowledgeBaseStats.mind + knowledgeBaseStats.body + knowledgeBaseStats.heart + knowledgeBaseStats.soul > 0) && (
            <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <div className="text-[9px] text-[var(--text-secondary)] uppercase font-bold mb-2">
                Knowledge Base
              </div>
              <div className="space-y-1.5">
                {[
                  { name: 'Mind', count: knowledgeBaseStats.mind },
                  { name: 'Body', count: knowledgeBaseStats.body },
                  { name: 'Heart', count: knowledgeBaseStats.heart },
                  { name: 'Soul', count: knowledgeBaseStats.soul },
                ].map((kb, i) => (
                  <div key={i} className="flex items-center justify-between text-[9px]">
                    <span className="text-[var(--text-secondary)]">{kb.name}</span>
                    <span className="text-[var(--bg-steel)] font-mono font-bold">
                      {kb.count} file{kb.count !== 1 ? 's' : ''}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default AttributionAnalytics;
