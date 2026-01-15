import React, { useState, useEffect } from 'react';
import { listAgents, killAgent, getAgentLogs, getAgentReport, AgentInfo, AgentReport } from '../services/agentService';

interface CrewListProps {
  twinId: string;
}

const CrewList: React.FC<CrewListProps> = ({ twinId }) => {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [maxAgents, setMaxAgents] = useState<number>(3);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [agentLogs, setAgentLogs] = useState<string[]>([]);
  const [agentReport, setAgentReport] = useState<AgentReport | null>(null);
  const [showLogs, setShowLogs] = useState<boolean>(false);

  const fetchAgents = async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await listAgents();
      setAgents(response.agents);
      setMaxAgents(response.max_agents);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to fetch agents';
      // When the browser throws a generic network error, add actionable context.
      // HTTP status/detail (e.g. 404/502 body) is handled by agentService.
      setError(
        msg === 'Failed to fetch'
          ? 'Failed to fetch agents: network error (Gateway/Orchestrator unreachable or blocked)'
          : msg
      );
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchAgents();
    // Refresh every 5 seconds
    const interval = setInterval(fetchAgents, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleKillAgent = async (agentId: string) => {
    if (!confirm(`Are you sure you want to terminate agent "${agentId}"?`)) {
      return;
    }
    try {
      await killAgent(agentId);
      await fetchAgents();
      if (selectedAgentId === agentId) {
        setSelectedAgentId(null);
        setAgentLogs([]);
        setAgentReport(null);
      }
    } catch (err) {
      alert(`Failed to kill agent: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const handleViewLogs = async (agentId: string) => {
    try {
      const [logsResponse, reportResponse] = await Promise.all([
        getAgentLogs(agentId),
        getAgentReport(agentId),
      ]);
      setAgentLogs(logsResponse.logs);
      setAgentReport(reportResponse.report);
      setSelectedAgentId(agentId);
      setShowLogs(true);
    } catch (err) {
      alert(`Failed to fetch agent details: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const formatDate = (dateString: string) => {
    try {
      return new Date(dateString).toLocaleString();
    } catch {
      return dateString;
    }
  };

  return (
    <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4">
      <div className="flex items-center justify-between gap-2 mb-4">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">groups</span>
          <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
            Active Crew ({agents.length}/{maxAgents})
          </h3>
        </div>
        <button
          onClick={fetchAgents}
          className="px-2 py-1 text-[10px] bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] text-[var(--text-on-accent)] rounded transition-all"
          title="Refresh"
        >
          <span className="material-symbols-outlined text-[14px]">refresh</span>
        </button>
      </div>

      {error && (
        <div className="mb-3 text-[11px] text-[rgb(var(--danger-rgb)/0.85)] bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--danger-rgb)/0.3)] rounded-lg px-3 py-2">
          {error}
        </div>
      )}

      {loading && agents.length === 0 ? (
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">Loading crew list...</div>
      ) : agents.length === 0 ? (
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">No active sub-agents. The Orchestrator can spawn workers for specialized tasks.</div>
      ) : (
        <>
          <div className="space-y-2 mb-4">
            {agents.map((agent) => (
              <div
                key={agent.agent_id}
                className="bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-3 hover:border-[var(--bg-muted)] transition-all"
              >
                <div className="flex items-start justify-between gap-2 mb-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-[11px] font-bold text-[var(--text-secondary)] truncate">{agent.name}</span>
                      <span className={`text-[9px] px-1.5 py-0.5 rounded font-bold uppercase ${
                        agent.status === 'idle' 
                          ? 'bg-[rgb(var(--bg-steel-rgb)/0.2)] text-[var(--bg-steel)]' 
                          : 'bg-[rgb(var(--bg-muted-rgb)/0.2)] text-[var(--bg-muted)]'
                      }`}>
                        {agent.status}
                      </span>
                    </div>
                    <div className="text-[10px] text-[var(--text-secondary)] opacity-80 mb-1 line-clamp-2">
                      {agent.mission}
                    </div>
                    <div className="text-[9px] text-[var(--bg-steel)] font-mono">
                      ID: {agent.agent_id.substring(0, 8)}...
                    </div>
                    <div className="text-[9px] text-[var(--bg-steel)] opacity-70 mt-1">
                      Created: {formatDate(agent.created_at)}
                    </div>
                    {agent.permissions.length > 0 && (
                      <div className="text-[9px] text-[var(--text-secondary)] opacity-70 mt-1">
                        Permissions: {agent.permissions.join(', ')}
                      </div>
                    )}
                  </div>
                  <div className="flex flex-col gap-1 shrink-0">
                    <button
                      onClick={() => handleViewLogs(agent.agent_id)}
                      className="px-2 py-1 text-[9px] bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] text-[var(--text-on-accent)] rounded transition-all"
                      title="View logs"
                    >
                      <span className="material-symbols-outlined text-[12px]">description</span>
                    </button>
                    <button
                      onClick={() => handleKillAgent(agent.agent_id)}
                      className="px-2 py-1 text-[9px] bg-[rgb(var(--danger-rgb)/0.95)] hover:bg-[rgb(var(--danger-rgb)/0.85)] text-[var(--text-on-accent)] rounded transition-all"
                      title="Terminate agent"
                    >
                      <span className="material-symbols-outlined text-[12px]">close</span>
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>

          {showLogs && selectedAgentId && (
            <div className="mt-4 border-t border-[rgb(var(--bg-steel-rgb)/0.3)] pt-4">
              <div className="flex items-center justify-between mb-2">
                <h4 className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                  Agent Logs & Report
                </h4>
                <button
                  onClick={() => {
                    setShowLogs(false);
                    setSelectedAgentId(null);
                  }}
                  className="text-[10px] text-[var(--bg-steel)] hover:text-[var(--text-secondary)]"
                >
                  Close
                </button>
              </div>

              {agentReport && (
                <div className="mb-3 bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-3">
                  <div className="text-[10px] font-bold text-[var(--text-secondary)] mb-1">Latest Report</div>
                  <div className="text-[9px] text-[var(--text-secondary)] opacity-80 mb-2">
                    Task: {agentReport.task}
                  </div>
                  <div className="text-[9px] text-[var(--text-secondary)] whitespace-pre-wrap font-mono bg-[rgb(var(--bg-secondary-rgb)/0.3)] p-2 rounded">
                    {agentReport.report}
                  </div>
                  <div className="text-[8px] text-[var(--bg-steel)] opacity-70 mt-1">
                    {formatDate(agentReport.created_at)}
                  </div>
                </div>
              )}

              {agentLogs.length > 0 && (
                <div className="bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-3 max-h-64 overflow-y-auto">
                  <div className="text-[10px] font-bold text-[var(--text-secondary)] mb-2">Activity Log</div>
                  <div className="space-y-1">
                    {agentLogs.map((log, idx) => (
                      <div key={idx} className="text-[9px] text-[var(--text-secondary)] font-mono opacity-80">
                        {log}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default CrewList;
