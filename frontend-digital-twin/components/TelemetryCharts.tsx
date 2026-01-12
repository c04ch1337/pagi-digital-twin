import React, { useMemo } from 'react';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend } from 'recharts';
import { TelemetryData } from '../types';

interface TelemetryChartsProps {
  data: TelemetryData[];
}

const TelemetryCharts: React.FC<TelemetryChartsProps> = ({ data }) => {
  // Format data for better visualization - show last 30 points for cleaner view
  const chartData = useMemo(() => {
    return data.slice(-30).map((point, index) => ({
      ...point,
      // Format timestamp to show just time (HH:MM:SS)
      time: point.timestamp ? point.timestamp.split(' ').pop() || point.timestamp : `${index}`,
      index, // For fallback if timestamp is missing
    }));
  }, [data]);

  // Custom tooltip with dark theme
  const CustomTooltip = ({ active, payload, label }: any) => {
    if (active && payload && payload.length) {
      return (
        <div className="bg-zinc-900 border border-indigo-500/50 rounded-lg p-2 shadow-xl">
          <p className="text-[10px] text-zinc-400 mb-1">{label}</p>
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
        <p className="text-[10px] text-zinc-600 italic">Waiting for telemetry data...</p>
      </div>
    );
  }

  return (
    <div className="w-full space-y-3">
      {/* CPU Usage Chart */}
      <div className="h-28">
        <div className="flex items-center justify-between mb-1">
          <h4 className="text-[10px] font-bold text-indigo-400 uppercase tracking-wider">CPU Usage</h4>
          <span className="text-[9px] text-zinc-500">
            {chartData[chartData.length - 1]?.cpu.toFixed(1) || 0}%
          </span>
        </div>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart 
            data={chartData} 
            margin={{ top: 2, right: 5, left: -15, bottom: 2 }}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="#27272a" opacity={0.3} />
            <XAxis 
              dataKey="time" 
              tickFormatter={formatXAxis}
              stroke="#71717a" 
              tick={{ fontSize: 9, fill: '#71717a' }}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            <YAxis 
              unit="%" 
              domain={[0, 100]} 
              stroke="#71717a" 
              tick={{ fontSize: 9, fill: '#71717a' }}
              width={30}
            />
            <Tooltip content={<CustomTooltip />} />
            <Line 
              type="monotone" 
              dataKey="cpu" 
              stroke="#6366f1" 
              strokeWidth={2} 
              dot={false}
              isAnimationActive={false}
              name="CPU"
            />
          </LineChart>
        </ResponsiveContainer>
      </div>

      {/* Memory Usage Chart */}
      <div className="h-28">
        <div className="flex items-center justify-between mb-1">
          <h4 className="text-[10px] font-bold text-emerald-400 uppercase tracking-wider">Memory Usage</h4>
          <span className="text-[9px] text-zinc-500">
            {chartData[chartData.length - 1]?.memory.toFixed(1) || 0}%
          </span>
        </div>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart 
            data={chartData} 
            margin={{ top: 2, right: 5, left: -15, bottom: 2 }}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="#27272a" opacity={0.3} />
            <XAxis 
              dataKey="time" 
              tickFormatter={formatXAxis}
              stroke="#71717a" 
              tick={{ fontSize: 9, fill: '#71717a' }}
              interval="preserveStartEnd"
              minTickGap={30}
            />
            <YAxis 
              unit="%" 
              domain={[0, 100]} 
              stroke="#71717a" 
              tick={{ fontSize: 9, fill: '#71717a' }}
              width={30}
            />
            <Tooltip content={<CustomTooltip />} />
            <Line 
              type="monotone" 
              dataKey="memory" 
              stroke="#10b981" 
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
        <div className="h-28">
          <div className="flex items-center justify-between mb-1">
            <h4 className="text-[10px] font-bold text-cyan-400 uppercase tracking-wider">Network</h4>
            <span className="text-[9px] text-zinc-500">
              {chartData[chartData.length - 1]?.network.toFixed(1) || 0}%
            </span>
          </div>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart 
              data={chartData} 
              margin={{ top: 2, right: 5, left: -15, bottom: 2 }}
            >
              <CartesianGrid strokeDasharray="3 3" stroke="#27272a" opacity={0.3} />
              <XAxis 
                dataKey="time" 
                tickFormatter={formatXAxis}
                stroke="#71717a" 
                tick={{ fontSize: 9, fill: '#71717a' }}
                interval="preserveStartEnd"
                minTickGap={30}
              />
              <YAxis 
                unit="%" 
                domain={[0, 100]} 
                stroke="#71717a" 
                tick={{ fontSize: 9, fill: '#71717a' }}
                width={30}
              />
              <Tooltip content={<CustomTooltip />} />
              <Line 
                type="monotone" 
                dataKey="network" 
                stroke="#06b6d4" 
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
