import React, { useState, useEffect } from 'react';
import { listScheduledTasks, ScheduledTask } from '../services/scheduledTasksService';
import { getToolProposals, ToolInstallationProposal } from '../services/toolProposalService';
import { getAllPeerReviews, PeerReview } from '../services/peerReviewService';
import { getAllRetrospectives, RetrospectiveAnalysis, applyPatch } from '../services/retrospectiveService';
import { getAllPersonas, AgentPersona } from '../services/personaService';

interface IntelligenceStreamProps {
  recentTopics: string[];
  hasActiveConsensusSession: boolean;
  onAuditLogClick?: (memoryNodeId: string) => void;
  onClear?: () => void;
}

interface StreamEvent {
  id: string;
  type: 'chronos' | 'audit' | 'tool' | 'memory' | 'consensus' | 'debate' | 'post_mortem';
  timestamp: Date;
  title: string;
  content: string;
  severity?: 'info' | 'warning' | 'critical';
  memoryNodeId?: string;
  agentStationId?: string;
  debate?: {
    review: PeerReview;
    requestingAgent: string;
    expertAgent: string;
    consensus?: 'approved' | 'rejected';
  };
  retrospective?: {
    analysis: RetrospectiveAnalysis;
    rootCause: string;
    hasPatch: boolean;
  };
}

const IntelligenceStream: React.FC<IntelligenceStreamProps> = ({
  recentTopics,
  hasActiveConsensusSession,
  onAuditLogClick,
  onClear,
}) => {
  const [events, setEvents] = useState<StreamEvent[]>([]);
  const [chronosTasks, setChronosTasks] = useState<ScheduledTask[]>([]);
  const [toolProposals, setToolProposals] = useState<ToolInstallationProposal[]>([]);
  const [peerReviews, setPeerReviews] = useState<PeerReview[]>([]);
  const [retrospectives, setRetrospectives] = useState<RetrospectiveAnalysis[]>([]);
  const [personas, setPersonas] = useState<Map<string, AgentPersona>>(new Map());
  const [loading, setLoading] = useState(false);

  // Fetch Chronos tasks
  const fetchChronosTasks = async () => {
    try {
      const tasks = await listScheduledTasks();
      setChronosTasks(tasks);
    } catch (err) {
      console.error('[IntelligenceStream] Failed to fetch Chronos tasks:', err);
    }
  };

  // Fetch tool proposals
  const fetchToolProposals = async () => {
    try {
      const response = await getToolProposals();
      setToolProposals(response.proposals.slice(0, 5)); // Latest 5
    } catch (err) {
      console.error('[IntelligenceStream] Failed to fetch tool proposals:', err);
    }
  };

  // Fetch audit findings (simplified - would come from audit API)
  const fetchAuditFindings = async () => {
    try {
      // Placeholder - would fetch from /api/phoenix/reports/latest or audit API
      // For now, we'll create synthetic events from recent topics
    } catch (err) {
      console.error('[IntelligenceStream] Failed to fetch audit findings:', err);
    }
  };

  // Fetch peer reviews
  const fetchPeerReviews = async () => {
    try {
      const response = await getAllPeerReviews();
      setPeerReviews(response.reviews.slice(0, 10)); // Latest 10
    } catch (err) {
      console.error('[IntelligenceStream] Failed to fetch peer reviews:', err);
    }
  };

  // Fetch retrospectives
  const fetchRetrospectives = async () => {
    try {
      const response = await getAllRetrospectives();
      setRetrospectives(response.retrospectives.slice(0, 10)); // Latest 10
    } catch (err) {
      console.error('[IntelligenceStream] Failed to fetch retrospectives:', err);
    }
  };

  useEffect(() => {
    const loadData = async () => {
      setLoading(true);
      await Promise.all([fetchChronosTasks(), fetchToolProposals(), fetchAuditFindings(), fetchPeerReviews(), fetchRetrospectives(), fetchPersonas()]);
      setLoading(false);
    };
    loadData();
    // Refresh every 10 seconds
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, []);

  // Merge all events into unified stream
  useEffect(() => {
    const mergedEvents: StreamEvent[] = [];

    // Add Chronos events (recent triggers)
    chronosTasks.forEach((task) => {
      if (task.last_run) {
        mergedEvents.push({
          id: `chronos-${task.id}`,
          type: 'chronos',
          timestamp: new Date(task.last_run),
          title: `Chronos Trigger: ${task.name}`,
          content: `Scheduled task "${task.name}" triggered at ${new Date(task.last_run).toLocaleTimeString()}`,
          severity: 'info',
        });
      }
    });

    // Add Tool Proposal events
    toolProposals.forEach((proposal) => {
      mergedEvents.push({
        id: `tool-${proposal.id}`,
        type: 'tool',
        timestamp: new Date(proposal.created_at),
        title: `Tool Proposal: ${proposal.tool_name}`,
        content: `${proposal.tool_name} - ${proposal.status} - ${proposal.description?.substring(0, 100) || 'No description'}...`,
        severity: proposal.status === 'pending' ? 'warning' : 'info',
      });
    });

    // Add Memory Transfer events
    recentTopics.forEach((topic, idx) => {
      mergedEvents.push({
        id: `memory-${topic}-${idx}`,
        type: 'memory',
        timestamp: new Date(),
        title: `Memory Transfer: ${topic}`,
        content: `Knowledge fragment transferred for topic: ${topic}`,
        severity: 'info',
        memoryNodeId: topic, // Simplified - would be actual node ID
      });
    });

    // Add Consensus events
    if (hasActiveConsensusSession) {
      mergedEvents.push({
        id: 'consensus-active',
        type: 'consensus',
        timestamp: new Date(),
        title: 'Consensus Session Active',
        content: 'Active consensus voting session detected across Agent Stations',
        severity: 'warning',
      });
    }

    // Add Peer Review Debate events
    peerReviews.forEach((review) => {
      const consensus = review.consensus || (review.expert_decision === 'concur' ? 'approved' : review.expert_decision === 'object' ? 'rejected' : undefined);
      mergedEvents.push({
        id: `debate-${review.review_id}`,
        type: 'debate',
        timestamp: new Date(review.created_at),
        title: `Agent Debate: ${review.tool_name}`,
        content: review.expert_decision 
          ? `${review.expert_agent_name}: ${review.expert_decision === 'concur' ? 'Concur' : 'Object'} - ${review.expert_reasoning?.substring(0, 80) || 'No reasoning'}...`
          : `Peer review requested: ${review.requesting_agent_name} → ${review.expert_agent_name}`,
        severity: consensus === 'rejected' ? 'warning' : consensus === 'approved' ? 'info' : 'warning',
        debate: {
          review,
          requestingAgent: review.requesting_agent_name,
          expertAgent: review.expert_agent_name,
          consensus,
        },
      });
    });

    // Add Post-Mortem Retrospective events
    retrospectives.forEach((retrospective) => {
      mergedEvents.push({
        id: `post-mortem-${retrospective.retrospective_id}`,
        type: 'post_mortem',
        timestamp: new Date(retrospective.failure_timestamp),
        title: `Post-Mortem: ${retrospective.tool_name}`,
        content: `Failure analysis for ${retrospective.tool_name}. Root cause: ${retrospective.root_cause}`,
        severity: 'critical',
        retrospective: {
          analysis: retrospective,
          rootCause: retrospective.root_cause,
          hasPatch: !!retrospective.suggested_patch,
        },
      });
    });

    // Sort by timestamp (newest first)
    mergedEvents.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());

    // Keep only latest 20 events
    setEvents(mergedEvents.slice(0, 20));
  }, [chronosTasks, toolProposals, recentTopics, hasActiveConsensusSession, peerReviews, retrospectives, personas]);

  const getEventIcon = (type: StreamEvent['type']) => {
    switch (type) {
      case 'chronos':
        return 'schedule';
      case 'audit':
        return 'assessment';
      case 'tool':
        return 'build';
      case 'memory':
        return 'memory';
      case 'consensus':
        return 'verified';
      case 'debate':
        return 'forum';
      case 'post_mortem':
        return 'bug_report';
      default:
        return 'info';
    }
  };

  const getEventColor = (type: StreamEvent['type'], severity?: StreamEvent['severity']) => {
    if (severity === 'critical') return 'text-[rgb(var(--danger-rgb))]';
    if (severity === 'warning') return 'text-[rgb(var(--warning-rgb))]';
    
    switch (type) {
      case 'chronos':
        return 'text-[var(--bg-steel)]';
      case 'audit':
        return 'text-[rgb(var(--warning-rgb))]';
      case 'tool':
        return 'text-[var(--accent)]';
      case 'memory':
        return 'text-[var(--success)]';
      case 'consensus':
        return 'text-[var(--bg-steel)]';
      case 'debate':
        return 'text-[var(--accent)]';
      case 'post_mortem':
        return 'text-[rgb(var(--danger-rgb))]';
      default:
        return 'text-[var(--text-secondary)]';
    }
  };

  const formatTime = (date: Date) => {
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSecs = Math.floor(diffMs / 1000);
    
    if (diffSecs < 60) return `${diffSecs}s ago`;
    if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
    if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`;
    return date.toLocaleDateString();
  };

  return (
    <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 h-full flex flex-col overflow-hidden">
      <div className="flex items-center justify-between gap-2 mb-3">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">stream</span>
          <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
            Intelligence Stream
          </h3>
        </div>
        <button
          onClick={onClear}
          className="px-2 py-1 text-[10px] bg-[rgb(var(--surface-rgb)/0.7)] hover:bg-[var(--bg-muted)] text-[var(--text-secondary)] rounded transition-all border border-[rgb(var(--bg-steel-rgb)/0.3)]"
          title="Clear stream"
        >
          Clear
        </button>
      </div>

      {loading && events.length === 0 ? (
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic text-center py-4">
          Loading intelligence stream...
        </div>
      ) : events.length === 0 ? (
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic text-center py-4">
          No recent activity. Intelligence stream will populate as events occur.
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto space-y-2">
          {events.map((event) => (
            <div
              key={event.id}
              className={`bg-[rgb(var(--bg-secondary-rgb)/0.25)] border border-[rgb(var(--bg-steel-rgb)/0.25)] rounded-lg p-2.5 transition-all hover:bg-[rgb(var(--bg-secondary-rgb)/0.4)] hover:border-[rgb(var(--bg-steel-rgb)/0.4)] ${
                event.memoryNodeId ? 'cursor-pointer' : ''
              }`}
              onClick={() => {
                if (event.memoryNodeId && onAuditLogClick) {
                  onAuditLogClick(event.memoryNodeId);
                }
              }}
            >
              <div className="flex items-start gap-2 mb-1">
                <span
                  className={`material-symbols-outlined text-[14px] ${getEventColor(event.type, event.severity)}`}
                >
                  {getEventIcon(event.type)}
                </span>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center justify-between gap-2 mb-0.5">
                    <div className="text-[10px] font-bold text-[var(--text-primary)] truncate">{event.title}</div>
                    <div className="text-[9px] text-[var(--text-secondary)] opacity-70 shrink-0">
                      {formatTime(event.timestamp)}
                    </div>
                  </div>
                  <div className="text-[9px] text-[var(--text-secondary)] opacity-80 line-clamp-2">
                    {event.content}
                  </div>
                  {event.type === 'debate' && event.debate && (
                    <div className="mt-2 space-y-1.5 border-t border-[rgb(var(--bg-steel-rgb)/0.2)] pt-1.5">
                      {(() => {
                        const expertPersona = event.debate.review.expert_agent_id ? personas.get(event.debate.review.expert_agent_id) : null;
                        const requestingPersona = event.debate.review.requesting_agent_id ? personas.get(event.debate.review.requesting_agent_id) : null;
                        return (
                          <>
                            <div className="text-[8px] text-[var(--text-secondary)] opacity-70">
                              <span className="font-semibold">
                                {requestingPersona ? `[${requestingPersona.name}] ` : ''}{event.debate.requestingAgent}:
                              </span> {event.debate.review.requesting_reasoning.substring(0, 100)}...
                              {requestingPersona && (
                                <div className="text-[7px] italic mt-0.5 opacity-60">
                                  Voice: {requestingPersona.voice_tone}
                                </div>
                              )}
                            </div>
                            {event.debate.review.expert_decision && (
                              <div className={`text-[8px] ${event.debate.review.expert_decision === 'concur' ? 'text-[var(--success)]' : 'text-[rgb(var(--warning-rgb))]'}`}>
                                <span className="font-semibold">
                                  {expertPersona ? `[${expertPersona.name}] ` : ''}{event.debate.expertAgent}:
                                </span> {event.debate.review.expert_reasoning?.substring(0, 100)}...
                                {expertPersona && (
                                  <div className="text-[7px] italic mt-0.5 opacity-60">
                                    Voice: {expertPersona.voice_tone}
                                  </div>
                                )}
                              </div>
                            )}
                          </>
                        );
                      })()}
                      {event.debate.consensus && (
                        <div className={`mt-1 px-1.5 py-0.5 rounded text-[8px] font-bold ${
                          event.debate.consensus === 'approved' 
                            ? 'bg-[var(--success)]/20 text-[var(--success)] border border-[var(--success)]/30' 
                            : 'bg-[rgb(var(--warning-rgb)/0.2)] text-[rgb(var(--warning-rgb))] border border-[rgb(var(--warning-rgb)/0.3)]'
                        }`}>
                          {event.debate.consensus === 'approved' ? '✓ Consensus: APPROVED' : '✗ Consensus: REJECTED'}
                        </div>
                      )}
                    </div>
                  )}
                  {event.type === 'audit' && event.memoryNodeId && (
                    <div className="mt-1 text-[8px] text-[var(--bg-steel)] italic">
                      Click to center Neural Map on related memory node
                    </div>
                  )}
                  {event.type === 'post_mortem' && event.retrospective && (
                    <div className="mt-2 space-y-1.5 border-t border-[rgb(var(--bg-steel-rgb)/0.2)] pt-1.5">
                      <div className="text-[8px] text-[var(--text-secondary)] opacity-70">
                        <span className="font-semibold">Root Cause:</span> {event.retrospective.rootCause}
                      </div>
                      <div className="text-[8px] text-[var(--text-secondary)] opacity-70">
                        <span className="font-semibold">Error Pattern:</span> {event.retrospective.analysis.error_pattern.substring(0, 100)}...
                      </div>
                      {event.retrospective.hasPatch && (
                        <>
                          <div className="mt-1 px-1.5 py-0.5 rounded text-[8px] font-bold bg-[var(--accent)]/20 text-[var(--accent)] border border-[var(--accent)]/30">
                            ✓ Patch Available
                          </div>
                          <button
                            onClick={async (e) => {
                              e.stopPropagation();
                              if (!confirm(`Apply patch to playbook "${event.retrospective!.analysis.tool_name}"? This will update the installation command and rehabilitate reliability by 2%.`)) {
                                return;
                              }
                              try {
                                const result = await applyPatch(event.retrospective!.analysis.retrospective_id);
                                if (result.ok) {
                                  alert(`Patch applied successfully! Playbook reliability rehabilitated.`);
                                  // Refresh retrospectives
                                  fetchRetrospectives();
                                } else {
                                  alert(`Failed to apply patch: ${result.message}`);
                                }
                              } catch (err) {
                                console.error('[IntelligenceStream] Failed to apply patch:', err);
                                alert('Failed to apply patch. Please check console for details.');
                              }
                            }}
                            className="mt-1 px-2 py-1 text-[8px] bg-[var(--success)] hover:bg-green-700 text-white rounded transition-colors font-bold"
                            title="Apply Patch (Rehabilitate reliability by 2%)"
                          >
                            Apply Patch
                          </button>
                        </>
                      )}
                      <div className="text-[8px] text-[rgb(var(--warning-rgb))]">
                        Reliability Impact: {(event.retrospective.analysis.reliability_impact * 100).toFixed(1)}%
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default IntelligenceStream;
