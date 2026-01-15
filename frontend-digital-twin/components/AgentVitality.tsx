import React, { useState, useEffect } from 'react';
import { listAgents, getAgentLogs, getAgentReport, AgentInfo, AgentReport } from '../services/agentService';
import { getMetricsStations, AgentStationMetrics as MetricsData } from '../services/metricsService';
import { getPersona, assignPersona, AgentPersona, BehavioralBias } from '../services/personaService';

interface AgentVitalityProps {
  twinId: string;
  onAgentStationClick?: (agentStationId: string) => void;
  selectedAgentStationId?: string | null;
}

interface AgentStationMetrics {
  reasoningLoad: number; // 0-100, based on active tasks and complexity
  driftFrequency: number; // 0-100, based on audit findings and corrections
  capabilityScore: number; // 0-100, based on historical success rates
  activeTasks: number;
  lastDriftTimestamp?: string;
}

const AgentVitality: React.FC<AgentVitalityProps> = ({ twinId, onAgentStationClick, selectedAgentStationId }) => {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [maxAgents, setMaxAgents] = useState<number>(3);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [agentMetrics, setAgentMetrics] = useState<Map<string, AgentStationMetrics>>(new Map());
  const [expandedStations, setExpandedStations] = useState<Set<string>>(new Set());
  const [agentPersonas, setAgentPersonas] = useState<Map<string, AgentPersona>>(new Map());
  const [personaModalAgentId, setPersonaModalAgentId] = useState<string | null>(null);
  const [personaForm, setPersonaForm] = useState<{
    name: string;
    voice_tone: string;
    behavioral_bias: BehavioralBias;
  }>({
    name: '',
    voice_tone: '',
    behavioral_bias: { cautiousness: 0.5, innovation: 0.5, detail_orientation: 0.5 },
  });

  const fetchAgents = async () => {
    try {
      setLoading(true);
      setError(null);
      
      // Fetch agents and metrics in parallel
      const [agentsResponse, metricsResponse] = await Promise.all([
        listAgents(),
        getMetricsStations().catch(() => ({ stations: [] })), // Gracefully handle metrics failure
      ]);
      
      setAgents(agentsResponse.agents);
      setMaxAgents(agentsResponse.max_agents);

      // Build metrics map from backend response
      const metricsMap = new Map<string, AgentStationMetrics>();
      
      // Create a map of metrics by agent_id for quick lookup
      const metricsByAgentId = new Map<string, MetricsData>();
      for (const station of metricsResponse.stations) {
        metricsByAgentId.set(station.agent_id, station);
      }

      // Map backend metrics to frontend format
      for (const agent of agentsResponse.agents) {
        const backendMetrics = metricsByAgentId.get(agent.agent_id);
        
        if (backendMetrics) {
          // Use real metrics from backend
          metricsMap.set(agent.agent_id, {
            reasoningLoad: backendMetrics.reasoning_load,
            driftFrequency: backendMetrics.drift_frequency,
            capabilityScore: backendMetrics.capability_score,
            activeTasks: backendMetrics.active_tasks,
            lastDriftTimestamp: backendMetrics.last_drift_timestamp,
          });
        } else {
          // Fallback to calculated values if metrics not available
          const reasoningLoad = agent.status === 'active' ? 75 : agent.status === 'idle' ? 10 : 50;
          metricsMap.set(agent.agent_id, {
            reasoningLoad,
            driftFrequency: 0,
            capabilityScore: 50,
            activeTasks: agent.status === 'active' ? 1 : 0,
          });
        }
      }
      
      setAgentMetrics(metricsMap);

      // Fetch personas for all agents
      const personaMap = new Map<string, AgentPersona>();
      for (const agent of agentsResponse.agents) {
        try {
          const personaResponse = await getPersona(agent.agent_id);
          if (personaResponse.success && personaResponse.persona) {
            personaMap.set(agent.agent_id, personaResponse.persona);
          }
        } catch (err) {
          // Silently fail persona fetch - not all agents have personas
        }
      }
      setAgentPersonas(personaMap);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to fetch agents';
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

  const getVitalityColor = (metrics: AgentStationMetrics): string => {
    // Nominal: Green (low drift, reasonable load)
    if (metrics.driftFrequency < 20 && metrics.reasoningLoad < 80) {
      return 'bg-[var(--success)]';
    }
    // In-Drift: Yellow (high drift or high load)
    if (metrics.driftFrequency >= 20 || metrics.reasoningLoad >= 80) {
      return 'bg-[rgb(var(--warning-rgb))]';
    }
    // Self-Healing: Pulsing Orange (recent drift correction)
    if (metrics.driftFrequency > 15 && metrics.driftFrequency < 30) {
      return 'bg-[rgb(var(--warning-rgb))]';
    }
    return 'bg-[rgb(var(--text-secondary-rgb)/0.35)]';
  };

  const getVitalityLabel = (metrics: AgentStationMetrics): string => {
    if (metrics.driftFrequency < 20 && metrics.reasoningLoad < 80) {
      return 'Nominal';
    }
    if (metrics.driftFrequency >= 20 || metrics.reasoningLoad >= 80) {
      return 'In-Drift';
    }
    if (metrics.driftFrequency > 15 && metrics.driftFrequency < 30) {
      return 'Self-Healing';
    }
    return 'Unknown';
  };

  const toggleStationExpansion = (agentId: string) => {
    setExpandedStations((prev) => {
      const next = new Set(prev);
      if (next.has(agentId)) {
        next.delete(agentId);
      } else {
        next.add(agentId);
      }
      return next;
    });
  };

  const formatDate = (dateString: string) => {
    try {
      return new Date(dateString).toLocaleString();
    } catch {
      return dateString;
    }
  };

  const handleAssignPersona = async (agentId: string) => {
    try {
      await assignPersona({
        agent_id: agentId,
        name: personaForm.name,
        voice_tone: personaForm.voice_tone,
        behavioral_bias: personaForm.behavioral_bias,
      });
      setPersonaModalAgentId(null);
      setPersonaForm({
        name: '',
        voice_tone: '',
        behavioral_bias: { cautiousness: 0.5, innovation: 0.5, detail_orientation: 0.5 },
      });
      // Refresh agents to get updated personas
      fetchAgents();
    } catch (err) {
      console.error('Failed to assign persona:', err);
      setError(err instanceof Error ? err.message : 'Failed to assign persona');
    }
  };

  const openPersonaModal = (agentId: string) => {
    const existingPersona = agentPersonas.get(agentId);
    if (existingPersona) {
      setPersonaForm({
        name: existingPersona.name,
        voice_tone: existingPersona.voice_tone,
        behavioral_bias: existingPersona.behavioral_bias,
      });
    }
    setPersonaModalAgentId(agentId);
  };

  return (
    <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 h-full flex flex-col overflow-hidden">
      <div className="flex items-center justify-between gap-2 mb-4">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">monitor_heart</span>
          <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
            Agent Vitality ({agents.length}/{maxAgents})
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

      <div className="flex-1 overflow-y-auto space-y-2">
        {loading && agents.length === 0 ? (
          <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">Loading agent stations...</div>
        ) : agents.length === 0 ? (
          <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">
            No active Agent Stations. The Orchestrator can spawn workers for specialized tasks.
          </div>
        ) : (
          agents.map((agent) => {
            const metrics = agentMetrics.get(agent.agent_id) || {
              reasoningLoad: 0,
              driftFrequency: 0,
              capabilityScore: 0,
              activeTasks: 0,
            };
            const isExpanded = expandedStations.has(agent.agent_id);
            const isSelected = selectedAgentStationId === agent.agent_id;
            const vitalityColor = getVitalityColor(metrics);
            const vitalityLabel = getVitalityLabel(metrics);

            return (
              <div
                key={agent.agent_id}
                className={`bg-[rgb(var(--surface-rgb)/0.7)] border-2 rounded-lg p-3 transition-all cursor-pointer ${
                  isSelected
                    ? 'border-[var(--bg-steel)] bg-[rgb(var(--bg-steel-rgb)/0.1)]'
                    : 'border-[rgb(var(--bg-steel-rgb)/0.3)] hover:border-[var(--bg-muted)]'
                }`}
                onClick={() => {
                  onAgentStationClick?.(agent.agent_id);
                  toggleStationExpansion(agent.agent_id);
                }}
              >
                {/* Station Header */}
                <div className="flex items-start justify-between gap-2 mb-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      {/* Expertise-based icon */}
                      <span className="material-symbols-outlined text-[14px] text-[var(--accent)]">
                        {(() => {
                          // Calculate expertise level based on capability score and metrics
                          // Scholars: >80% capability, Experts: 60-80%, Practitioners: 40-60%, Novices: <40%
                          if (metrics.capabilityScore > 80) return 'school'; // Scholar
                          if (metrics.capabilityScore > 60) return 'psychology'; // Expert
                          if (metrics.capabilityScore > 40) return 'engineering'; // Practitioner
                          return 'smart_toy'; // Novice
                        })()}
                      </span>
                      <span className="text-[11px] font-bold text-[var(--text-secondary)] truncate">{agent.name}</span>
                      {agentPersonas.get(agent.agent_id) && (
                        <span className="text-[9px] px-1.5 py-0.5 rounded bg-[var(--bg-steel)] text-[var(--text-on-accent)] italic">
                          {agentPersonas.get(agent.agent_id)?.name}
                        </span>
                      )}
                      <span
                        className={`text-[9px] px-1.5 py-0.5 rounded font-bold uppercase ${
                          vitalityColor
                        } text-white`}
                        style={
                          vitalityLabel === 'Self-Healing'
                            ? { animation: 'pulse 2s ease-in-out infinite' }
                            : undefined
                        }
                      >
                        {vitalityLabel}
                      </span>
                    </div>
                    <div className="text-[10px] text-[var(--text-secondary)] opacity-80 mb-1 line-clamp-2">
                      {agent.mission}
                    </div>
                    {agentPersonas.get(agent.agent_id) && (
                      <div className="text-[9px] text-[var(--bg-steel)] italic mb-1">
                        Voice: {agentPersonas.get(agent.agent_id)?.voice_tone}
                      </div>
                    )}
                    <div className="text-[9px] text-[var(--bg-steel)] font-mono">
                      Station: {agent.agent_id.substring(0, 8)}...
                    </div>
                  </div>
                </div>

                {/* Vitality Metrics */}
                <div className="space-y-1.5 mt-2 pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                  {/* Reasoning Load */}
                  <div>
                    <div className="flex items-center justify-between mb-0.5">
                      <span className="text-[9px] text-[var(--text-secondary)] uppercase">Reasoning Load</span>
                      <span className="text-[9px] font-bold text-[var(--text-primary)]">
                        {metrics.reasoningLoad.toFixed(0)}%
                      </span>
                    </div>
                    <div className="w-full bg-[rgb(var(--bg-secondary-rgb)/1)] rounded-full h-1.5 overflow-hidden">
                      <div
                        className={`h-full transition-all ${
                          metrics.reasoningLoad > 80
                            ? 'bg-[rgb(var(--warning-rgb))]'
                            : metrics.reasoningLoad > 50
                            ? 'bg-[var(--bg-steel)]'
                            : 'bg-[var(--success)]'
                        }`}
                        style={{ width: `${metrics.reasoningLoad}%` }}
                      />
                    </div>
                  </div>

                  {/* Drift Frequency */}
                  <div>
                    <div className="flex items-center justify-between mb-0.5">
                      <span className="text-[9px] text-[var(--text-secondary)] uppercase">Drift Frequency</span>
                      <span className="text-[9px] font-bold text-[var(--text-primary)]">
                        {metrics.driftFrequency.toFixed(1)}%
                      </span>
                    </div>
                    <div className="w-full bg-[rgb(var(--bg-secondary-rgb)/1)] rounded-full h-1.5 overflow-hidden">
                      <div
                        className={`h-full transition-all ${
                          metrics.driftFrequency > 30
                            ? 'bg-[rgb(var(--danger-rgb))]'
                            : metrics.driftFrequency > 15
                            ? 'bg-[rgb(var(--warning-rgb))]'
                            : 'bg-[var(--success)]'
                        }`}
                        style={{ width: `${Math.min(100, metrics.driftFrequency * 2)}%` }}
                      />
                    </div>
                  </div>

                  {/* Capability Score */}
                  <div>
                    <div className="flex items-center justify-between mb-0.5">
                      <span className="text-[9px] text-[var(--text-secondary)] uppercase">Capability</span>
                      <span className="text-[9px] font-bold text-[var(--text-primary)]">
                        {metrics.capabilityScore.toFixed(0)}%
                      </span>
                    </div>
                    <div className="w-full bg-[rgb(var(--bg-secondary-rgb)/1)] rounded-full h-1.5 overflow-hidden">
                      <div
                        className="h-full bg-[var(--bg-steel)] transition-all"
                        style={{ width: `${metrics.capabilityScore}%` }}
                      />
                    </div>
                  </div>
                </div>

                {/* Expanded Details */}
                {isExpanded && (
                  <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                    <div className="text-[9px] text-[var(--text-secondary)] space-y-1">
                      <div>
                        <span className="font-semibold">Status:</span> {agent.status}
                      </div>
                      <div>
                        <span className="font-semibold">Active Tasks:</span> {metrics.activeTasks}
                      </div>
                      <div>
                        <span className="font-semibold">Created:</span> {formatDate(agent.created_at)}
                      </div>
                      {agent.permissions.length > 0 && (
                        <div>
                          <span className="font-semibold">Permissions:</span> {agent.permissions.join(', ')}
                        </div>
                      )}
                      {agentPersonas.get(agent.agent_id) && (
                        <div>
                          <span className="font-semibold">Persona:</span> {agentPersonas.get(agent.agent_id)?.name}
                          <div className="mt-1 text-[8px] opacity-80">
                            Cautiousness: {(agentPersonas.get(agent.agent_id)!.behavioral_bias.cautiousness * 100).toFixed(0)}% | 
                            Innovation: {(agentPersonas.get(agent.agent_id)!.behavioral_bias.innovation * 100).toFixed(0)}% | 
                            Detail: {(agentPersonas.get(agent.agent_id)!.behavioral_bias.detail_orientation * 100).toFixed(0)}%
                          </div>
                        </div>
                      )}
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          openPersonaModal(agent.agent_id);
                        }}
                        className="mt-2 px-2 py-1 text-[9px] bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] text-[var(--text-on-accent)] rounded transition-all"
                      >
                        {agentPersonas.get(agent.agent_id) ? 'Edit Persona' : 'Assign Persona'}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Persona Assignment Modal */}
      {personaModalAgentId && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setPersonaModalAgentId(null)}>
          <div className="bg-[rgb(var(--surface-rgb))] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4 max-w-md w-full mx-4" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-sm font-bold mb-3 text-[var(--text-primary)]">Assign Persona</h3>
            <div className="space-y-3">
              <div>
                <label className="text-[10px] text-[var(--text-secondary)] uppercase">Persona Name</label>
                <input
                  type="text"
                  value={personaForm.name}
                  onChange={(e) => setPersonaForm({ ...personaForm, name: e.target.value })}
                  placeholder="e.g., The Architect, The Skeptic"
                  className="w-full px-2 py-1 text-[11px] bg-[rgb(var(--bg-secondary-rgb))] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded text-[var(--text-primary)]"
                />
              </div>
              <div>
                <label className="text-[10px] text-[var(--text-secondary)] uppercase">Voice Tone</label>
                <input
                  type="text"
                  value={personaForm.voice_tone}
                  onChange={(e) => setPersonaForm({ ...personaForm, voice_tone: e.target.value })}
                  placeholder="e.g., Socratic and inquisitive"
                  className="w-full px-2 py-1 text-[11px] bg-[rgb(var(--bg-secondary-rgb))] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded text-[var(--text-primary)]"
                />
              </div>
              <div>
                <label className="text-[10px] text-[var(--text-secondary)] uppercase">Cautiousness: {(personaForm.behavioral_bias.cautiousness * 100).toFixed(0)}%</label>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.1"
                  value={personaForm.behavioral_bias.cautiousness}
                  onChange={(e) => setPersonaForm({
                    ...personaForm,
                    behavioral_bias: { ...personaForm.behavioral_bias, cautiousness: parseFloat(e.target.value) }
                  })}
                  className="w-full"
                />
              </div>
              <div>
                <label className="text-[10px] text-[var(--text-secondary)] uppercase">Innovation: {(personaForm.behavioral_bias.innovation * 100).toFixed(0)}%</label>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.1"
                  value={personaForm.behavioral_bias.innovation}
                  onChange={(e) => setPersonaForm({
                    ...personaForm,
                    behavioral_bias: { ...personaForm.behavioral_bias, innovation: parseFloat(e.target.value) }
                  })}
                  className="w-full"
                />
              </div>
              <div>
                <label className="text-[10px] text-[var(--text-secondary)] uppercase">Detail Orientation: {(personaForm.behavioral_bias.detail_orientation * 100).toFixed(0)}%</label>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.1"
                  value={personaForm.behavioral_bias.detail_orientation}
                  onChange={(e) => setPersonaForm({
                    ...personaForm,
                    behavioral_bias: { ...personaForm.behavioral_bias, detail_orientation: parseFloat(e.target.value) }
                  })}
                  className="w-full"
                />
              </div>
              <div className="flex gap-2 justify-end mt-4">
                <button
                  onClick={() => setPersonaModalAgentId(null)}
                  className="px-3 py-1.5 text-[10px] bg-[rgb(var(--bg-secondary-rgb))] hover:bg-[rgb(var(--bg-steel-rgb)/0.5)] text-[var(--text-secondary)] rounded transition-all"
                >
                  Cancel
                </button>
                <button
                  onClick={() => handleAssignPersona(personaModalAgentId)}
                  disabled={!personaForm.name.trim() || !personaForm.voice_tone.trim()}
                  className="px-3 py-1.5 text-[10px] bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] text-[var(--text-on-accent)] rounded transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Assign
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default AgentVitality;
