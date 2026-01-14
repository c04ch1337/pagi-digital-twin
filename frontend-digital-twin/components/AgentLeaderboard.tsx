import React, { useState, useEffect } from 'react';
import { listAgents, AgentInfo } from '../services/agentService';
import HoverTooltip from './HoverTooltip';

interface AgentMetrics {
  agent_id: string;
  name: string;
  commits: number;
  efficiency: number; // token-to-result ratio (higher is better)
  durability: number; // how many times playbooks have been called
  badges: string[];
}

interface AgentLeaderboardProps {
  onInspectAgent?: (agentId: string) => void;
  refreshInterval?: number;
}

const AgentLeaderboard: React.FC<AgentLeaderboardProps> = ({ 
  onInspectAgent,
  refreshInterval = 10000 
}) => {
  const [metrics, setMetrics] = useState<AgentMetrics[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchLeaderboard = async () => {
    try {
      setLoading(true);
      setError(null);
      
      // Fetch agents list
      const agentsResponse = await listAgents();
      
      // Fetch leaderboard metrics from backend
      const gatewayUrl = import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
      const leaderboardResponse = await fetch(`${gatewayUrl}/api/agents/leaderboard`, {
        method: 'GET',
        headers: {
          Accept: 'application/json',
        },
      });

      let leaderboardData: Record<string, Partial<AgentMetrics>> = {};
      
      if (leaderboardResponse.ok) {
        const data = await leaderboardResponse.json();
        leaderboardData = data.metrics || {};
      }

      // Combine agent info with metrics
      const combinedMetrics: AgentMetrics[] = agentsResponse.agents.map(agent => {
        const agentMetrics = leaderboardData[agent.agent_id] || {};
        const badges: string[] = [];
        
        // Calculate badges
        if (agentMetrics.durability && agentMetrics.durability >= 10) {
          badges.push('Legacy Builder');
        }
        if (agentMetrics.commits && agentMetrics.commits >= 50) {
          badges.push('Safety Guard');
        }
        
        return {
          agent_id: agent.agent_id,
          name: agent.name,
          commits: agentMetrics.commits || 0,
          efficiency: agentMetrics.efficiency || 0,
          durability: agentMetrics.durability || 0,
          badges: [...badges, ...(agentMetrics.badges || [])],
        };
      });

      // Sort by combined score (commits + efficiency + durability)
      combinedMetrics.sort((a, b) => {
        const scoreA = a.commits * 10 + a.efficiency * 5 + a.durability * 3;
        const scoreB = b.commits * 10 + b.efficiency * 5 + b.durability * 3;
        return scoreB - scoreA;
      });

      setMetrics(combinedMetrics);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch leaderboard';
      setError(errorMessage);
      console.error('[AgentLeaderboard] Error:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchLeaderboard();
    const interval = setInterval(fetchLeaderboard, refreshInterval);
    return () => clearInterval(interval);
  }, [refreshInterval]);

  if (loading && metrics.length === 0) {
    return (
      <div className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 shadow-lg">
        <div className="text-center py-8">
          <div className="text-sm text-[var(--text-secondary)] mb-2">Loading leaderboard...</div>
          <div className="w-8 h-8 border-4 border-[var(--bg-steel)] border-t-transparent rounded-full animate-spin mx-auto"></div>
        </div>
      </div>
    );
  }

  if (error && metrics.length === 0) {
    return (
      <div className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--danger-rgb)/0.3)] rounded-xl p-4 shadow-lg">
        <div className="text-center py-4">
          <div className="text-sm text-[rgb(var(--danger-rgb)/0.85)] mb-2">Error loading leaderboard</div>
          <div className="text-xs text-[var(--text-secondary)] mb-4">{error}</div>
          <button
            onClick={fetchLeaderboard}
            className="px-4 py-2 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  const top3 = metrics.slice(0, 3);
  const rest = metrics.slice(3);

  const getBadgeColor = (badge: string): string => {
    switch (badge) {
      case 'Safety Guard':
        return 'bg-[rgb(var(--success-rgb)/0.8)]';
      case 'Legacy Builder':
        return 'bg-[rgb(var(--info-rgb)/0.8)]';
      default:
        return 'bg-[rgb(var(--bg-steel-rgb)/0.8)]';
    }
  };

  const getPodiumHeight = (position: number): string => {
    switch (position) {
      case 0: return 'h-24'; // 1st place - tallest
      case 1: return 'h-20'; // 2nd place
      case 2: return 'h-16'; // 3rd place
      default: return 'h-12';
    }
  };

  const getPodiumColor = (position: number): string => {
    switch (position) {
      case 0: return 'bg-[rgb(var(--warning-rgb)/0.8)]'; // Gold
      case 1: return 'bg-[rgb(var(--surface-rgb)/0.28)]'; // Silver
      case 2: return 'bg-[rgb(var(--accent-rgb)/0.65)]'; // Bronze
      default: return 'bg-[rgb(var(--bg-steel-rgb)/0.8)]';
    }
  };

  return (
    <div className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6 shadow-lg">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">emoji_events</span>
          <h3 className="text-sm font-bold text-[var(--text-secondary)] uppercase tracking-wider">
            Phoenix Leaderboard
          </h3>
        </div>
        <button
          onClick={fetchLeaderboard}
          className="px-3 py-1.5 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all flex items-center gap-1"
        >
          <span className="material-symbols-outlined text-sm">refresh</span>
          Refresh
        </button>
      </div>

      {/* Podium for Top 3 */}
      {top3.length > 0 && (
        <div className="mb-6">
          <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-wider mb-3 text-center">
            Top Performers
          </div>
          <div className="flex items-end justify-center gap-4">
            {top3.map((agent, idx) => {
              const position = idx;
              // Podium order: 2nd place (middle), 1st place (left), 3rd place (right)
              const podiumOrder = [1, 0, 2]; // Visual order: [2nd, 1st, 3rd]
              
              return (
                <div
                  key={agent.agent_id}
                  className="flex flex-col items-center"
                  style={{ order: podiumOrder[idx] }}
                >
                  {/* Medal/Crown */}
                  <div className="mb-2">
                    {position === 0 && (
                      <span className="material-symbols-outlined text-[rgb(var(--warning-rgb)/0.9)] text-2xl">
                        emoji_events
                      </span>
                    )}
                    {position === 1 && (
                      <span className="material-symbols-outlined text-[var(--text-muted)] text-xl">
                        emoji_events
                      </span>
                    )}
                    {position === 2 && (
                      <span className="material-symbols-outlined text-[rgb(var(--accent-rgb)/0.9)] text-lg">
                        emoji_events
                      </span>
                    )}
                  </div>
                  
                  {/* Podium */}
                  <div
                    className={`w-20 ${getPodiumHeight(position)} ${getPodiumColor(position)} rounded-t-lg flex flex-col items-center justify-end pb-2 shadow-lg border-2 border-[rgb(var(--bg-steel-rgb)/0.3)]`}
                  >
                    <div className="text-xs font-bold text-[var(--text-on-accent)] text-center px-2 truncate w-full">
                      {agent.name}
                    </div>
                    <div className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.9)] mt-1">
                      #{position + 1}
                    </div>
                  </div>
                  
                  {/* Metrics */}
                  <div className="mt-2 text-center">
                    <div className="text-[9px] text-[var(--text-secondary)] font-bold">
                      {agent.commits} commits
                    </div>
                    <div className="text-[8px] text-[var(--bg-steel)]">
                      Eff: {agent.efficiency.toFixed(1)}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Full Leaderboard Table */}
      <div className="space-y-2">
        <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-wider mb-2">
          All Agents
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead className="bg-[rgb(var(--bg-muted-rgb)/0.3)]">
              <tr>
                <th className="text-left p-2 text-[var(--text-secondary)] font-bold">Rank</th>
                <th className="text-left p-2 text-[var(--text-secondary)] font-bold">Agent</th>
                <th className="text-right p-2 text-[var(--text-secondary)] font-bold">Commits</th>
                <th className="text-right p-2 text-[var(--text-secondary)] font-bold">Efficiency</th>
                <th className="text-right p-2 text-[var(--text-secondary)] font-bold">Durability</th>
                <th className="text-center p-2 text-[var(--text-secondary)] font-bold">Badges</th>
                <th className="text-center p-2 text-[var(--text-secondary)] font-bold">Action</th>
              </tr>
            </thead>
            <tbody>
              {metrics.map((agent, idx) => (
                <tr
                  key={agent.agent_id}
                  className="border-b border-[rgb(var(--bg-steel-rgb)/0.2)] hover:bg-[rgb(var(--bg-secondary-rgb)/0.2)] transition-colors"
                >
                  <td className="p-2 text-[var(--text-secondary)] font-bold">#{idx + 1}</td>
                  <td className="p-2 text-[var(--text-secondary)] font-mono text-[10px]">{agent.name}</td>
                  <td className="p-2 text-right text-[var(--text-secondary)] font-mono">{agent.commits}</td>
                  <td className="p-2 text-right text-[var(--text-secondary)] font-mono">
                    {agent.efficiency.toFixed(2)}
                  </td>
                  <td className="p-2 text-right text-[var(--text-secondary)] font-mono">{agent.durability}</td>
                  <td className="p-2 text-center">
                    <div className="flex flex-wrap gap-1 justify-center">
                      {agent.badges.map((badge, badgeIdx) => (
                        <HoverTooltip
                          key={badgeIdx}
                          title={badge}
                          description={`Achievement badge for ${badge}`}
                        >
                          <span
                            className={`px-2 py-0.5 rounded-full text-[8px] font-bold text-[var(--text-on-accent)] ${getBadgeColor(badge)}`}
                          >
                            {badge}
                          </span>
                        </HoverTooltip>
                      ))}
                      {agent.badges.length === 0 && (
                        <span className="text-[8px] text-[var(--bg-steel)]">—</span>
                      )}
                    </div>
                  </td>
                  <td className="p-2 text-center">
                    <button
                      onClick={() => onInspectAgent?.(agent.agent_id)}
                      className="px-2 py-1 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] rounded text-[9px] font-bold text-[var(--text-on-accent)] transition-all flex items-center gap-1 mx-auto"
                      title="Inspect Agent Brain"
                    >
                      <span className="material-symbols-outlined text-xs">psychology</span>
                      Inspect
                    </button>
                  </td>
                </tr>
              ))}
              {metrics.length === 0 && (
                <tr>
                  <td colSpan={7} className="p-4 text-center text-[var(--bg-steel)]">
                    No agents found
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

export default AgentLeaderboard;
