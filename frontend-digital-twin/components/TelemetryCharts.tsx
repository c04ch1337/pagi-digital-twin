import React, { useMemo } from 'react';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend } from 'recharts';
import { TelemetryData } from '../types';
import HoverTooltip from './HoverTooltip';

// NOTE:
// Recharts v3 includes internal prop-reporting logic that can trigger an infinite
// render loop if we pass new object/function identities on every render.
// Keep commonly reused objects/functions stable (module scope) to avoid
// `Maximum update depth exceeded` crashes.

const CHART_MARGIN = { top: 2, right: 5, left: -15, bottom: 2 } as const;
const X_TICK_STYLE = { fontSize: 9, fill: 'var(--text-secondary)' } as const;
const Y_TICK_STYLE = { fontSize: 9, fill: 'var(--text-secondary)' } as const;

// Format timestamp for X-axis (show only time portion)
const formatXAxis = (tickItem: string) => {
  if (!tickItem) return '';
  // If it's already formatted as time, return it
  if (tickItem.includes(':')) {
    const parts = tickItem.split(':');
    if (parts.length >= 2) {
      return `${parts[parts.length - 2]}:${parts[parts.length - 1]}`;
    }
  }
  return tickItem;
};

// Custom tooltip with light tactical theme
const TelemetryTooltip = ({ active, payload, label }: any) => {
  if (active && payload && payload.length) {
    return (
      <div className="bg-[rgb(var(--surface-rgb)/0.9)] border border-[rgb(var(--bg-steel-rgb)/0.4)] rounded-lg p-2 shadow-xl">
        <p className="text-[10px] text-[var(--text-secondary)] mb-1">{label}</p>
        {payload.map((entry: any, index: number) => (
          <p key={index} className="text-xs font-semibold" style={{ color: entry.color }}>
            {entry.name}: {entry.value}%
          </p>
        ))}
      </div>
    );
  }
  return null;
};

interface TelemetryChartsProps {
  data: TelemetryData[];
}

const TelemetryCharts: React.FC<TelemetryChartsProps> = ({ data }) => {
  // Format data for better visualization - show last 30 points for cleaner view
  const chartData = useMemo(() => {
    const toPercent = (v: unknown): number => {
      const n = typeof v === 'number' ? v : Number(v);
      if (!Number.isFinite(n)) return 0;

      // Support either 0..1 or 0..100 inputs.
      const percent = n <= 1 ? n * 100 : n;
      return Math.max(0, Math.min(100, percent));
    };

    return data.slice(-30).map((point, index) => ({
      ...point,
      cpu: toPercent(point.cpu),
      memory: toPercent(point.memory),
      network: toPercent((point as any).network),
      // Format timestamp to show just the time portion.
      // TelemetryContext uses `toLocaleTimeString()` (e.g. "1:23:45 PM").
      // The previous implementation used `.split(' ').pop()` which would yield only "PM".
      time: (() => {
        const raw = point.timestamp ? String(point.timestamp) : `${index}`;

        // If locale time with AM/PM: take the time part.
        const parts = raw.split(' ');
        if (parts.length >= 1 && parts[0].includes(':')) {
          return parts[0];
        }

        // If ISO-ish: try to take the last token.
        if (raw.includes('T')) {
          const maybeTime = raw.split('T').pop() ?? raw;
          return maybeTime.split('Z')[0];
        }

        return raw;
      })(),
      index, // For fallback if timestamp is missing
    }));
  }, [data]);

  if (chartData.length === 0) {
    return (
      <div className="h-32 w-full flex items-center justify-center">
        <HoverTooltip
          title="Telemetry"
          description="No samples have arrived yet. Ensure the Telemetry service is running and the UI has an active SSE connection."
        >
          <p className="text-[10px] text-[var(--text-secondary)] italic cursor-help">Waiting for telemetry data...</p>
        </HoverTooltip>
      </div>
    );
  }

  return (
    <div className="w-full space-y-3">
      {/* CPU Usage Chart */}
      <HoverTooltip
        title="CPU Usage Chart"
        description="CPU utilization history (last ~30 samples, 0–100%). Hover the line to inspect the value at a point in time."
      >
        <div className="h-28 cursor-help">
        <div className="flex items-center justify-between mb-1">
          <HoverTooltip
            title="CPU Usage"
            description="CPU utilization (%). Higher values indicate more compute load across cores."
          >
            <h4 className="text-[10px] font-bold text-[rgb(var(--bg-steel-rgb)/0.9)] uppercase tracking-wider cursor-help">CPU Usage</h4>
          </HoverTooltip>
          <HoverTooltip
            title="Latest CPU"
            description="Most recent CPU utilization sample."
          >
            <span className="text-[9px] text-[rgb(var(--text-secondary-rgb)/0.7)] cursor-help">
              {chartData[chartData.length - 1]?.cpu.toFixed(1) || 0}%
            </span>
          </HoverTooltip>
        </div>
        <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={0}>
          <LineChart 
            data={chartData} 
            margin={CHART_MARGIN}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="rgb(var(--accent-rgb))" opacity={0.18} />
            <XAxis 
              dataKey="time" 
              tickFormatter={formatXAxis}
              stroke="var(--text-secondary)" 
              tick={X_TICK_STYLE}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            <YAxis 
              unit="%" 
              domain={[0, 100]} 
              stroke="var(--text-secondary)" 
              tick={Y_TICK_STYLE}
              width={30}
            />
            <Tooltip content={TelemetryTooltip} />
            <Line 
              type="monotone" 
              dataKey="cpu" 
              stroke="var(--accent)" 
              strokeWidth={2} 
              dot={false}
              isAnimationActive={false}
              name="CPU"
            />
          </LineChart>
        </ResponsiveContainer>
        </div>
      </HoverTooltip>

      {/* Memory Usage Chart */}
      <HoverTooltip
        title="Memory Usage Chart"
        description="Memory utilization history (last ~30 samples, 0–100%). Hover the line to inspect the value at a point in time."
      >
        <div className="h-28 cursor-help">
        <div className="flex items-center justify-between mb-1">
          <HoverTooltip
            title="Memory Usage"
            description="Memory utilization (%). Higher values indicate increased RAM pressure."
          >
            <h4 className="text-[10px] font-bold text-[var(--success)] uppercase tracking-wider cursor-help">Memory Usage</h4>
          </HoverTooltip>
          <HoverTooltip
            title="Latest Memory"
            description="Most recent memory utilization sample."
          >
            <span className="text-[9px] text-[rgb(var(--text-secondary-rgb)/0.7)] cursor-help">
              {chartData[chartData.length - 1]?.memory.toFixed(1) || 0}%
            </span>
          </HoverTooltip>
        </div>
        <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={0}>
          <LineChart 
            data={chartData} 
            margin={CHART_MARGIN}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="rgb(var(--accent-rgb))" opacity={0.18} />
            <XAxis 
              dataKey="time" 
              tickFormatter={formatXAxis}
              stroke="var(--text-secondary)" 
              tick={X_TICK_STYLE}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            <YAxis 
              unit="%" 
              domain={[0, 100]} 
              stroke="var(--text-secondary)" 
              tick={Y_TICK_STYLE}
              width={30}
            />
            <Tooltip content={TelemetryTooltip} />
            <Line 
              type="monotone" 
              dataKey="memory" 
              stroke="rgb(var(--success-rgb))" 
              strokeWidth={2} 
              dot={false}
              isAnimationActive={false}
              name="Memory"
            />
          </LineChart>
        </ResponsiveContainer>
        </div>
      </HoverTooltip>

      {/* Network Usage Chart (if available) */}
      {chartData.some(d => d.network !== undefined && d.network > 0) && (
        <HoverTooltip
          title="Network Activity Chart"
          description="Normalized network activity history (last ~30 samples, 0–100%). Hover the line to inspect point values."
        >
          <div className="h-28 cursor-help">
          <div className="flex items-center justify-between mb-1">
            <HoverTooltip
              title="Network"
              description="Network activity (%). This is a normalized indicator from the telemetry service."
            >
              <h4 className="text-[10px] font-bold text-[var(--info)] uppercase tracking-wider cursor-help">Network</h4>
            </HoverTooltip>
            <HoverTooltip
              title="Latest Network"
              description="Most recent network activity sample."
            >
              <span className="text-[9px] text-[rgb(var(--text-secondary-rgb)/0.7)] cursor-help">
                {chartData[chartData.length - 1]?.network.toFixed(1) || 0}%
              </span>
            </HoverTooltip>
          </div>
          <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={0}>
            <LineChart 
              data={chartData} 
              margin={CHART_MARGIN}
            >
              <CartesianGrid strokeDasharray="3 3" stroke="rgb(var(--accent-rgb))" opacity={0.18} />
              <XAxis 
                dataKey="time" 
                tickFormatter={formatXAxis}
                stroke="var(--text-secondary)" 
                tick={X_TICK_STYLE}
                interval="preserveStartEnd"
                minTickGap={30}
              />
              <YAxis 
                unit="%" 
                domain={[0, 100]} 
                stroke="var(--text-secondary)" 
                tick={Y_TICK_STYLE}
                width={30}
              />
              <Tooltip content={TelemetryTooltip} />
              <Line 
                type="monotone" 
                dataKey="network" 
                stroke="var(--info)" 
                strokeWidth={2} 
                dot={false}
                isAnimationActive={false}
                name="Network"
              />
            </LineChart>
          </ResponsiveContainer>
          </div>
        </HoverTooltip>
      )}
    </div>
  );
};

export default TelemetryCharts;
