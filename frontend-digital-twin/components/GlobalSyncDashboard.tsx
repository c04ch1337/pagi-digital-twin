import React, { useState, useEffect, useCallback } from 'react';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend } from 'recharts';
import { fetchTrafficData, calculateNetworkHealth, GitHubCloneData, GitHubViewData } from '../services/githubService';
import HoverTooltip from './HoverTooltip';

interface GlobalSyncDashboardProps {
  githubToken?: string;
  refreshInterval?: number;
}

const GlobalSyncDashboard: React.FC<GlobalSyncDashboardProps> = ({
  githubToken,
  refreshInterval = 60000, // 1 minute default
}) => {
  const [clones, setClones] = useState<GitHubCloneData[]>([]);
  const [views, setViews] = useState<GitHubViewData[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [networkHealth, setNetworkHealth] = useState<number>(0);
  const [pulseFrequency, setPulseFrequency] = useState<number>(0);

  const fetchData = useCallback(async () => {
    if (!githubToken) {
      setError('GitHub token not configured. Set GITHUB_TOKEN environment variable.');
      setLoading(false);
      return;
    }

    try {
      setError(null);
      const { clones: cloneData, views: viewData } = await fetchTrafficData(githubToken);
      
      // Filter to last 14 days
      const fourteenDaysAgo = new Date();
      fourteenDaysAgo.setDate(fourteenDaysAgo.getDate() - 14);
      
      const filteredClones = cloneData.filter(item => {
        const itemDate = new Date(item.timestamp);
        return itemDate >= fourteenDaysAgo;
      });
      
      const filteredViews = viewData.filter(item => {
        const itemDate = new Date(item.timestamp);
        return itemDate >= fourteenDaysAgo;
      });

      setClones(filteredClones);
      setViews(filteredViews);

      // Calculate network health
      const health = calculateNetworkHealth(filteredClones, filteredViews);
      setNetworkHealth(health);

      // Calculate pulse frequency (average unique clones per day)
      const avgClonesPerDay = filteredClones.length > 0
        ? filteredClones.reduce((sum, day) => sum + day.uniques, 0) / filteredClones.length
        : 0;
      setPulseFrequency(Math.round(avgClonesPerDay));
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch GitHub traffic data';
      setError(errorMessage);
      console.error('[GlobalSyncDashboard] Error:', err);
    } finally {
      setLoading(false);
    }
  }, [githubToken]);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, refreshInterval);
    return () => clearInterval(interval);
  }, [fetchData, refreshInterval]);

  // Prepare chart data
  const chartData = React.useMemo(() => {
    // Combine clones and views by date
    const dataMap = new Map<string, { date: string; clones: number; views: number }>();

    clones.forEach(item => {
      const date = new Date(item.timestamp).toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
      if (!dataMap.has(date)) {
        dataMap.set(date, { date, clones: 0, views: 0 });
      }
      const entry = dataMap.get(date)!;
      entry.clones = item.uniques;
    });

    views.forEach(item => {
      const date = new Date(item.timestamp).toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
      if (!dataMap.has(date)) {
        dataMap.set(date, { date, clones: 0, views: 0 });
      }
      const entry = dataMap.get(date)!;
      entry.views = item.uniques;
    });

    return Array.from(dataMap.values()).sort((a, b) => {
      return new Date(a.date).getTime() - new Date(b.date).getTime();
    });
  }, [clones, views]);

  const getHealthColor = (health: number): string => {
    if (health >= 70) return 'text-[var(--success)]';
    if (health >= 40) return 'text-[rgb(var(--warning-rgb)/0.9)]';
    return 'text-[rgb(var(--danger-rgb)/0.9)]';
  };

  const getHealthBgColor = (health: number): string => {
    if (health >= 70) return 'bg-[rgb(var(--success-rgb)/0.2)] border-[rgb(var(--success-rgb)/0.5)]';
    if (health >= 40) return 'bg-[rgb(var(--warning-rgb)/0.2)] border-[rgb(var(--warning-rgb)/0.5)]';
    return 'bg-[rgb(var(--danger-rgb)/0.2)] border-[rgb(var(--danger-rgb)/0.5)]';
  };

  if (loading && clones.length === 0) {
    return (
      <div className="bg-gradient-to-br from-[rgb(var(--overlay-rgb)/0.85)] via-[rgb(var(--overlay-rgb)/0.7)] to-[rgb(var(--overlay-rgb)/0.85)] border border-[rgb(var(--info-rgb)/0.3)] rounded-xl p-6 shadow-2xl relative overflow-hidden">
        {/* Cyber-grid background */}
        <div className="absolute inset-0 opacity-10" style={{
          backgroundImage: `
            linear-gradient(rgb(var(--info-rgb)) 1px, transparent 1px),
            linear-gradient(90deg, rgb(var(--info-rgb)) 1px, transparent 1px)
          `,
          backgroundSize: '20px 20px',
        }} />
        
        <div className="relative z-10 text-center py-8">
          <div className="text-sm text-[var(--info)] mb-2">Loading network sync data...</div>
          <div className="w-8 h-8 border-4 border-[var(--info)] border-t-transparent rounded-full animate-spin mx-auto"></div>
        </div>
      </div>
    );
  }

  if (error && clones.length === 0) {
    return (
      <div className="bg-gradient-to-br from-[rgb(var(--overlay-rgb)/0.85)] via-[rgb(var(--overlay-rgb)/0.7)] to-[rgb(var(--overlay-rgb)/0.85)] border border-[rgb(var(--danger-rgb)/0.3)] rounded-xl p-6 shadow-2xl relative overflow-hidden">
        <div className="absolute inset-0 opacity-10" style={{
          backgroundImage: `
            linear-gradient(rgb(var(--danger-rgb)) 1px, transparent 1px),
            linear-gradient(90deg, rgb(var(--danger-rgb)) 1px, transparent 1px)
          `,
          backgroundSize: '20px 20px',
        }} />
        
        <div className="relative z-10 text-center py-4">
          <div className="text-sm text-[rgb(var(--danger-rgb)/0.8)] mb-2">Error loading network data</div>
          <div className="text-xs text-[rgb(var(--info-rgb)/0.9)] mb-4">{error}</div>
          <button
            onClick={fetchData}
            className="px-4 py-2 bg-[rgb(var(--info-rgb)/0.75)] hover:bg-[var(--info)] rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-gradient-to-br from-[rgb(var(--overlay-rgb)/0.85)] via-[rgb(var(--overlay-rgb)/0.7)] to-[rgb(var(--overlay-rgb)/0.85)] border border-[rgb(var(--info-rgb)/0.3)] rounded-xl p-6 shadow-2xl relative overflow-hidden">
      {/* Cyber-grid background */}
      <div className="absolute inset-0 opacity-10" style={{
        backgroundImage: `
          linear-gradient(rgb(var(--info-rgb)) 1px, transparent 1px),
          linear-gradient(90deg, rgb(var(--info-rgb)) 1px, transparent 1px)
        `,
        backgroundSize: '20px 20px',
      }} />

      <div className="relative z-10">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-[var(--info)]">sync</span>
            <h3 className="text-sm font-bold text-[rgb(var(--info-rgb)/0.9)] uppercase tracking-wider">
              Global Sync Dashboard
            </h3>
          </div>
          <button
            onClick={fetchData}
            className="px-3 py-1.5 bg-[rgb(var(--info-rgb)/0.6)] hover:bg-[var(--info)] rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all flex items-center gap-1"
          >
            <span className="material-symbols-outlined text-sm">refresh</span>
            Refresh
          </button>
        </div>

        {/* Network Health Indicator */}
        <div className={`mb-6 p-4 rounded-lg border-2 ${getHealthBgColor(networkHealth)}`}>
          <div className="flex items-center justify-between">
            <div>
              <div className="text-xs text-[rgb(var(--info-rgb)/0.72)] mb-1">Network Health</div>
              <div className={`text-2xl font-bold ${getHealthColor(networkHealth)}`}>
                {networkHealth}%
              </div>
            </div>
            <div className="text-right">
              <div className="text-xs text-[rgb(var(--info-rgb)/0.72)] mb-1">Active Nodes (14d)</div>
              <div className="text-lg font-bold text-[var(--info)]">
                {clones.reduce((sum, day) => sum + day.uniques, 0)}
              </div>
            </div>
          </div>
        </div>

        {/* Pulse Frequency Indicator */}
        <div className="mb-6 p-4 bg-[rgb(var(--overlay-rgb)/0.25)] border border-[rgb(var(--info-rgb)/0.3)] rounded-lg">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className="w-3 h-3 bg-[var(--info)] rounded-full animate-pulse"></div>
              <span className="text-xs text-[rgb(var(--info-rgb)/0.72)]">Pulse Frequency</span>
            </div>
            <div className="text-lg font-bold text-[var(--info)]">
              {pulseFrequency} syncs/day
            </div>
          </div>
          <div className="text-[10px] text-[rgb(var(--info-rgb)/0.6)] mt-2">
            Average manifest.yaml syncs per day
          </div>
        </div>

        {/* Clone Map Chart */}
        <div className="mb-4">
          <div className="text-xs font-bold text-[rgb(var(--info-rgb)/0.9)] uppercase tracking-wider mb-3">
            Active Nodes (Clones) - Last 14 Days
          </div>
          <ResponsiveContainer width="100%" height={200}>
            <LineChart data={chartData}>
              <CartesianGrid strokeDasharray="3 3" stroke="rgb(var(--info-rgb))" opacity={0.2} />
              <XAxis
                dataKey="date"
                stroke="rgb(var(--info-rgb))"
                style={{ fontSize: '10px' }}
                tick={{ fill: 'rgb(var(--info-rgb) / 0.85)' }}
              />
              <YAxis
                stroke="rgb(var(--info-rgb))"
                style={{ fontSize: '10px' }}
                tick={{ fill: 'rgb(var(--info-rgb) / 0.85)' }}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: 'rgb(var(--overlay-rgb) / 0.85)',
                  border: '1px solid rgb(var(--info-rgb) / 0.8)',
                  borderRadius: '8px',
                  color: 'rgb(var(--info-rgb) / 0.9)',
                }}
              />
              <Legend
                wrapperStyle={{ fontSize: '10px', color: 'rgb(var(--info-rgb) / 0.9)' }}
              />
              <Line
                type="monotone"
                dataKey="clones"
                stroke="rgb(var(--info-rgb))"
                strokeWidth={2}
                dot={{ fill: 'rgb(var(--info-rgb))', r: 3 }}
                name="Unique Clones"
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Stats Grid */}
        <div className="grid grid-cols-2 gap-4 mt-4">
          <div className="bg-[rgb(var(--overlay-rgb)/0.25)] border border-[rgb(var(--info-rgb)/0.3)] rounded-lg p-3">
            <div className="text-[10px] text-[rgb(var(--info-rgb)/0.72)] mb-1">Total Clones</div>
            <div className="text-lg font-bold text-[var(--info)]">
              {clones.reduce((sum, day) => sum + day.count, 0)}
            </div>
          </div>
          <div className="bg-[rgb(var(--overlay-rgb)/0.25)] border border-[rgb(var(--info-rgb)/0.3)] rounded-lg p-3">
            <div className="text-[10px] text-[rgb(var(--info-rgb)/0.72)] mb-1">Total Views</div>
            <div className="text-lg font-bold text-[var(--info)]">
              {views.reduce((sum, day) => sum + day.count, 0)}
            </div>
          </div>
        </div>

        {/* Info Note */}
        {!githubToken && (
          <div className="mt-4 p-3 bg-[rgb(var(--warning-rgb)/0.2)] border border-[rgb(var(--warning-rgb)/0.5)] rounded-lg">
            <div className="text-xs text-[rgb(var(--warning-rgb)/0.9)]">
              <strong>Note:</strong> Configure GITHUB_TOKEN environment variable with a fine-grained
              Personal Access Token (Administration: Read permissions) to enable this dashboard.
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default GlobalSyncDashboard;
