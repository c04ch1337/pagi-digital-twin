import React, { useMemo } from 'react';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend } from 'recharts';
import { TelemetryData } from '../types';

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

  // Custom tooltip with dark theme
  const CustomTooltip = ({ active, payload, label }: any) => {
    if (active && payload && payload.length) {
      return (
        <div className="bg-white/90 border border-[#5381A5]/40 rounded-lg p-2 shadow-xl">
          <p className="text-[10px] text-[#163247] mb-1">{label}</p>
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

  if (chartData.length === 0) {
    return (
      <div className="h-32 w-full flex items-center justify-center">
        <p className="text-[10px] text-[#163247] italic">Waiting for telemetry data...</p>
      </div>
    );
  }

  return (
    <div className="w-full space-y-3">
      {/* CPU Usage Chart */}
      <div className="h-28" title="CPU chart: last ~30 samples (0–100%). Hover the line for point details.">
        <div className="flex items-center justify-between mb-1">
          <h4
            className="text-[10px] font-bold text-indigo-400 uppercase tracking-wider"
            title="CPU utilization (%). Higher values indicate more compute load across cores."
          >
            CPU Usage
          </h4>
          <span className="text-[9px] text-zinc-500" title="Latest CPU utilization sample">
            {chartData[chartData.length - 1]?.cpu.toFixed(1) || 0}%
          </span>
        </div>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart 
            data={chartData} 
            margin={{ top: 2, right: 5, left: -15, bottom: 2 }}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="#5381A5" opacity={0.18} />
            <XAxis 
              dataKey="time" 
              tickFormatter={formatXAxis}
              stroke="#163247" 
              tick={{ fontSize: 9, fill: '#163247' }}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            <YAxis 
              unit="%" 
              domain={[0, 100]} 
              stroke="#163247" 
              tick={{ fontSize: 9, fill: '#163247' }}
              width={30}
            />
            <Tooltip content={<CustomTooltip />} />
            <Line 
              type="monotone" 
              dataKey="cpu" 
              stroke="#5381A5" 
              strokeWidth={2} 
              dot={false}
              isAnimationActive={false}
              name="CPU"
            />
          </LineChart>
        </ResponsiveContainer>
      </div>

      {/* Memory Usage Chart */}
      <div className="h-28" title="Memory chart: last ~30 samples (0–100%). Hover the line for point details.">
        <div className="flex items-center justify-between mb-1">
          <h4
            className="text-[10px] font-bold text-emerald-400 uppercase tracking-wider"
            title="Memory utilization (%). Higher values indicate increased RAM pressure."
          >
            Memory Usage
          </h4>
          <span className="text-[9px] text-zinc-500" title="Latest memory utilization sample">
            {chartData[chartData.length - 1]?.memory.toFixed(1) || 0}%
          </span>
        </div>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart 
            data={chartData} 
            margin={{ top: 2, right: 5, left: -15, bottom: 2 }}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="#5381A5" opacity={0.18} />
            <XAxis 
              dataKey="time" 
              tickFormatter={formatXAxis}
              stroke="#163247" 
              tick={{ fontSize: 9, fill: '#163247' }}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            <YAxis 
              unit="%" 
              domain={[0, 100]} 
              stroke="#163247" 
              tick={{ fontSize: 9, fill: '#163247' }}
              width={30}
            />
            <Tooltip content={<CustomTooltip />} />
            <Line 
              type="monotone" 
              dataKey="memory" 
              stroke="#78A2C2" 
              strokeWidth={2} 
              dot={false}
              isAnimationActive={false}
              name="Memory"
            />
          </LineChart>
        </ResponsiveContainer>
      </div>

      {/* Network Usage Chart (if available) */}
      {chartData.some(d => d.network !== undefined && d.network > 0) && (
        <div className="h-28" title="Network chart: last ~30 samples (0–100%). Hover the line for point details.">
          <div className="flex items-center justify-between mb-1">
            <h4
              className="text-[10px] font-bold text-cyan-400 uppercase tracking-wider"
              title="Network activity (%). This is a normalized indicator from the telemetry service."
            >
              Network
            </h4>
            <span className="text-[9px] text-zinc-500" title="Latest network activity sample">
              {chartData[chartData.length - 1]?.network.toFixed(1) || 0}%
            </span>
          </div>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart 
              data={chartData} 
              margin={{ top: 2, right: 5, left: -15, bottom: 2 }}
            >
              <CartesianGrid strokeDasharray="3 3" stroke="#5381A5" opacity={0.18} />
              <XAxis 
                dataKey="time" 
                tickFormatter={formatXAxis}
                stroke="#163247" 
                tick={{ fontSize: 9, fill: '#163247' }}
                interval="preserveStartEnd"
                minTickGap={30}
              />
              <YAxis 
                unit="%" 
                domain={[0, 100]} 
                stroke="#163247" 
                tick={{ fontSize: 9, fill: '#163247' }}
                width={30}
              />
              <Tooltip content={<CustomTooltip />} />
              <Line 
                type="monotone" 
                dataKey="network" 
                stroke="#90C3EA" 
                strokeWidth={2} 
                dot={false}
                isAnimationActive={false}
                name="Network"
              />
            </LineChart>
          </ResponsiveContainer>
        </div>
      )}
    </div>
  );
};

export default TelemetryCharts;
