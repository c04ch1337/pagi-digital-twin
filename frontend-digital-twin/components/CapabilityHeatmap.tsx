import React, { useState, useEffect } from 'react';
import { getToolProposals, ToolInstallationProposal } from '../services/toolProposalService';
import { listAgents, AgentInfo } from '../services/agentService';
import { getTopPlaybooksForAgent, Playbook } from '../services/playbookService';

interface CapabilityHeatmapProps {
  className?: string;
}

interface CapabilityData {
  agentId: string;
  agentName: string;
  language: string;
  successCount: number;
  totalCount: number;
  expertiseLevel: number; // 0-100
}

const CapabilityHeatmap: React.FC<CapabilityHeatmapProps> = ({ className = '' }) => {
  const [capabilities, setCapabilities] = useState<Map<string, CapabilityData>>(new Map());
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [playbooks, setPlaybooks] = useState<Map<string, Playbook[]>>(new Map()); // key: "agentId:language"
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchData = async () => {
      try {
        setLoading(true);
        setError(null);

        // Fetch agents and tool proposals in parallel
        const [agentsResponse, proposalsResponse] = await Promise.all([
          listAgents(),
          getToolProposals(),
        ]);

        setAgents(agentsResponse.agents);

        // Build capability matrix: Agent x Language -> Success Rate
        const capabilityMap = new Map<string, CapabilityData>();

        // Initialize with all agents and common languages
        const commonLanguages = ['Python', 'Rust', 'JavaScript', 'TypeScript', 'Go', 'Java', 'Shell', 'Other'];
        
        for (const agent of agentsResponse.agents) {
          for (const lang of commonLanguages) {
            const key = `${agent.agent_id}:${lang}`;
            capabilityMap.set(key, {
              agentId: agent.agent_id,
              agentName: agent.name,
              language: lang,
              successCount: 0,
              totalCount: 0,
              expertiseLevel: 0,
            });
          }
        }

        // Process tool proposals to calculate expertise
        for (const proposal of proposalsResponse.proposals) {
          const lang = proposal.language || 'Other';
          const key = `${proposal.agent_id}:${lang}`;
          
          if (capabilityMap.has(key)) {
            const data = capabilityMap.get(key)!;
            data.totalCount += 1;
            
            // Count as successful if approved and verified
            if (proposal.status === 'approved' && proposal.verified === true) {
              data.successCount += 1;
            }
            
            // Calculate expertise level (success rate * 100)
            if (data.totalCount > 0) {
              data.expertiseLevel = (data.successCount / data.totalCount) * 100;
            }
          } else {
            // Create new entry if agent/language combo doesn't exist
            const successCount = proposal.status === 'approved' && proposal.verified === true ? 1 : 0;
            capabilityMap.set(key, {
              agentId: proposal.agent_id,
              agentName: proposal.agent_name,
              language: lang,
              successCount,
              totalCount: 1,
              expertiseLevel: successCount * 100,
            });
          }
        }

        setCapabilities(capabilityMap);

        // Fetch playbooks for each agent/language combination
        const playbookMap = new Map<string, Playbook[]>();
        for (const agent of agentsResponse.agents) {
          for (const lang of commonLanguages) {
            const key = `${agent.agent_id}:${lang}`;
            try {
              const topPlaybooks = await getTopPlaybooksForAgent(agent.agent_id, lang, 3);
              if (topPlaybooks.length > 0) {
                playbookMap.set(key, topPlaybooks);
              }
            } catch (err) {
              // Silently fail for playbook fetching
              console.debug('[CapabilityHeatmap] Failed to fetch playbooks for', key, err);
            }
          }
        }
        setPlaybooks(playbookMap);
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to fetch capability data';
        setError(msg);
        console.error('[CapabilityHeatmap] Failed to fetch data:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    // Refresh every 30 seconds
    const interval = setInterval(fetchData, 30000);
    return () => clearInterval(interval);
  }, []);

  // Get unique languages from capabilities
  const languages = Array.from(new Set(Array.from(capabilities.values()).map(c => c.language))).sort();

  // Get expertise level for a specific agent and language
  const getExpertiseLevel = (agentId: string, language: string): number => {
    const key = `${agentId}:${language}`;
    return capabilities.get(key)?.expertiseLevel || 0;
  };

  // Get color intensity based on expertise level
  const getExpertiseColor = (level: number): string => {
    if (level === 0) {
      return 'bg-[rgb(var(--bg-secondary-rgb)/0.2)]'; // No data
    }
    if (level >= 80) {
      return 'bg-[var(--success)]'; // Expert (green)
    }
    if (level >= 50) {
      return 'bg-[var(--bg-steel)]'; // Proficient (blue)
    }
    if (level >= 25) {
      return 'bg-[rgb(var(--warning-rgb))]'; // Learning (yellow)
    }
    return 'bg-[rgb(var(--danger-rgb)/0.5)]'; // Novice (red)
  };

  // Get tooltip text
  const getTooltipText = (agentId: string, language: string): string => {
    const key = `${agentId}:${language}`;
    const data = capabilities.get(key);
    const agentPlaybooks = playbooks.get(key) || [];
    
    let tooltip = '';
    if (data) {
      tooltip = `${data.agentName} - ${language}\nSuccess: ${data.successCount}/${data.totalCount} (${data.expertiseLevel.toFixed(0)}%)`;
    } else {
      tooltip = `${language}: No data`;
    }
    
    // Add top playbooks if available
    if (agentPlaybooks.length > 0) {
      tooltip += `\n\n📚 Top Playbooks:`;
      agentPlaybooks.slice(0, 3).forEach((pb, idx) => {
        tooltip += `\n${idx + 1}. ${pb.tool_name} (${(pb.reliability_score * 100).toFixed(0)}% reliable)`;
      });
    }
    
    return tooltip;
  };

  if (loading && capabilities.size === 0) {
    return (
      <div className={`${className} flex items-center justify-center h-full`}>
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">
          Loading capability heatmap...
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`${className} flex items-center justify-center h-full`}>
        <div className="text-[11px] text-[rgb(var(--danger-rgb)/0.85)] bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--danger-rgb)/0.3)] rounded-lg px-3 py-2">
          {error}
        </div>
      </div>
    );
  }

  if (agents.length === 0) {
    return (
      <div className={`${className} flex items-center justify-center h-full`}>
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">
          No agent stations available
        </div>
      </div>
    );
  }

  return (
    <div className={`${className} h-full flex flex-col overflow-hidden`}>
      <div className="mb-3">
        <h4 className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
          Capability Heatmap
        </h4>
        <p className="text-[9px] text-[var(--text-secondary)] opacity-70">
          Expertise level by Agent Station × Language (based on successful tool deployments)
        </p>
      </div>

      <div className="flex-1 overflow-auto">
        <div className="min-w-full">
          {/* Header row with languages */}
          <div className="sticky top-0 bg-[rgb(var(--surface-rgb)/0.9)] z-10 border-b border-[rgb(var(--bg-steel-rgb)/0.3)]">
            <div className="grid gap-1 p-2" style={{ gridTemplateColumns: `120px repeat(${languages.length}, minmax(60px, 1fr))` }}>
              <div className="text-[9px] font-bold uppercase text-[var(--text-secondary)]">
                Agent
              </div>
              {languages.map((lang) => (
                <div
                  key={lang}
                  className="text-[9px] font-bold uppercase text-[var(--text-secondary)] text-center"
                  title={lang}
                >
                  {lang.length > 8 ? lang.substring(0, 8) + '...' : lang}
                </div>
              ))}
            </div>
          </div>

          {/* Data rows */}
          <div className="divide-y divide-[rgb(var(--bg-steel-rgb)/0.2)]">
            {agents.map((agent) => (
              <div
                key={agent.agent_id}
                className="grid gap-1 p-2 hover:bg-[rgb(var(--surface-rgb)/0.5)] transition-colors"
                style={{ gridTemplateColumns: `120px repeat(${languages.length}, minmax(60px, 1fr))` }}
              >
                {/* Agent name */}
                <div className="text-[10px] font-semibold text-[var(--text-primary)] truncate" title={agent.name}>
                  {agent.name.length > 15 ? agent.name.substring(0, 15) + '...' : agent.name}
                </div>

                {/* Expertise cells */}
                {languages.map((lang) => {
                  const level = getExpertiseLevel(agent.agent_id, lang);
                  const color = getExpertiseColor(level);
                  const tooltip = getTooltipText(agent.agent_id, lang);

                  return (
                    <div
                      key={`${agent.agent_id}:${lang}`}
                      className={`${color} rounded transition-all cursor-help flex items-center justify-center min-h-[24px]`}
                      title={tooltip}
                    >
                      {level > 0 && (
                        <span className="text-[8px] font-bold text-white">
                          {level.toFixed(0)}%
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Legend */}
      <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
        <div className="flex items-center gap-4 flex-wrap">
          <div className="flex items-center gap-1.5">
            <div className="w-4 h-4 bg-[var(--success)] rounded"></div>
            <span className="text-[9px] text-[var(--text-secondary)]">Expert (80%+)</span>
          </div>
          <div className="flex items-center gap-1.5">
            <div className="w-4 h-4 bg-[var(--bg-steel)] rounded"></div>
            <span className="text-[9px] text-[var(--text-secondary)]">Proficient (50-79%)</span>
          </div>
          <div className="flex items-center gap-1.5">
            <div className="w-4 h-4 bg-[rgb(var(--warning-rgb))] rounded"></div>
            <span className="text-[9px] text-[var(--text-secondary)]">Learning (25-49%)</span>
          </div>
          <div className="flex items-center gap-1.5">
            <div className="w-4 h-4 bg-[rgb(var(--danger-rgb)/0.5)] rounded"></div>
            <span className="text-[9px] text-[var(--text-secondary)]">Novice (&lt;25%)</span>
          </div>
          <div className="flex items-center gap-1.5">
            <div className="w-4 h-4 bg-[rgb(var(--bg-secondary-rgb)/0.2)] rounded"></div>
            <span className="text-[9px] text-[var(--text-secondary)]">No data</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default CapabilityHeatmap;
