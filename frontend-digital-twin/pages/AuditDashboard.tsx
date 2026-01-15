import React, { useState, useEffect, useMemo } from 'react';
import PhoenixRecoveryConsole from '../components/PhoenixRecoveryConsole';
import { listAgents, getAgentLogs } from '../services/agentService';

interface ConflictProfile {
  node_id: string;
  compliance_score: number;
  approved: boolean;
  timestamp: string;
}

interface GovernanceReportEntry {
  agent_id: string;
  commit_hash: string;
  override_timestamp: string;
  rationale: string;
  conflict_profile: ConflictProfile[];
  redacted_count_at_override: number;
  knowledge_fragments_since: number;
  impact_summary: string;
}

interface StrategicRecommendation {
  entry_index: number;
  recommendation: string;
}

interface GovernanceReport {
  generated_at: string;
  total_overrides: number;
  entries: GovernanceReportEntry[];
  strategic_recommendations?: StrategicRecommendation[];
}

interface DraftRule {
  id: string;
  rule_type: {
    PythonRegex?: { pattern: string; target: string };
    RustFilter?: { module: string; function: string };
    ConfigUpdate?: { key: string; value: string };
  };
  description: string;
  proposed_change: string;
  source_recommendations: number[];
  confidence: number;
}

interface DraftRulesResponse {
  drafts: DraftRule[];
  total_recommendations_analyzed: number;
}

interface TopicInfo {
  topic: string;
  frequency: number;
  lastAccess?: Date;
  decayPercentage: number;
  status: 'active' | 'decaying' | 'critical';
}

interface AuditDashboardProps {
  onClose?: () => void;
}

const AuditDashboard: React.FC<AuditDashboardProps> = ({ onClose }) => {
  const [activeTab, setActiveTab] = useState<'report' | 'timeline' | 'memory' | 'audit-logs'>('report');
  const [reportMarkdown, setReportMarkdown] = useState<string>('');
  const [reportJson, setReportJson] = useState<GovernanceReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [topics, setTopics] = useState<TopicInfo[]>([]);
  const [selectedTopics, setSelectedTopics] = useState<Set<string>>(new Set());
  const [pruning, setPruning] = useState(false);
  const [snapshotStatus, setSnapshotStatus] = useState<{ hasRecent: boolean; lastSnapshotTime?: string } | null>(null);
  const [creatingSnapshot, setCreatingSnapshot] = useState(false);
  const [showRecoveryConsole, setShowRecoveryConsole] = useState(false);
  const [draftRules, setDraftRules] = useState<DraftRule[]>([]);
  const [loadingDrafts, setLoadingDrafts] = useState(false);
  const [applyingRule, setApplyingRule] = useState<string | null>(null);
  const [auditLogs, setAuditLogs] = useState<string[]>([]);
  const [loadingAuditLogs, setLoadingAuditLogs] = useState(false);
  const [phoenixAuditorAgentId, setPhoenixAuditorAgentId] = useState<string | null>(null);
  const [pathTrends, setPathTrends] = useState<Map<string, { changeCount: number; recurring: boolean; severityEscalation: boolean }>>(new Map());
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [pathHistory, setPathHistory] = useState<any[]>([]);
  const [loadingPathHistory, setLoadingPathHistory] = useState(false);

  // Fetch governance report
  useEffect(() => {
    const fetchReport = async () => {
      setLoading(true);
      setError(null);
      try {
        // Fetch both markdown and JSON versions
        const [markdownRes, jsonRes] = await Promise.all([
          fetch('/api/phoenix/reports/latest'),
          fetch('/api/phoenix/reports/latest.json'),
        ]);

        if (!markdownRes.ok || !jsonRes.ok) {
          throw new Error('Failed to fetch governance report');
        }

        const markdown = await markdownRes.text();
        const json: GovernanceReport = await jsonRes.json();

        setReportMarkdown(markdown);
        setReportJson(json);
      } catch (err) {
        console.error('[AuditDashboard] Failed to fetch report:', err);
        setError(err instanceof Error ? err.message : 'Failed to load governance report');
      } finally {
        setLoading(false);
      }
    };

    fetchReport();
    fetchDraftRules();
    fetchPhoenixAuditorLogs();
  }, []);

  // Extract file paths from audit log text
  const extractPathsFromLogs = (logs: string[]): string[] => {
    const paths = new Set<string>();
    // Common path patterns
    const pathPatterns = [
      /(?:^|\s)(\/[^\s]+)/g,  // Unix paths
      /(?:^|\s)([A-Z]:\\[^\s]+)/g,  // Windows paths
      /(?:^|\s)(C:\\[^\s]+)/gi,  // Windows C: paths
    ];
    
    logs.forEach(log => {
      pathPatterns.forEach(pattern => {
        const matches = log.matchAll(pattern);
        for (const match of matches) {
          if (match[1] && match[1].length > 3) {
            paths.add(match[1]);
          }
        }
      });
    });
    
    return Array.from(paths);
  };

  // Fetch trend data for a specific path
  const fetchPathTrend = async (path: string) => {
    try {
      const response = await fetch(`/api/audit/trends?path=${encodeURIComponent(path)}&days=30`);
      if (response.ok) {
        const data = await response.json();
        if (data.ok && data.trend) {
          setPathTrends(prev => new Map(prev).set(path, {
            changeCount: data.trend.change_count,
            recurring: data.trend.recurring_climax,
            severityEscalation: data.trend.severity_escalation,
          }));
        }
      }
    } catch (err) {
      console.error(`[AuditDashboard] Failed to fetch trend for ${path}:`, err);
    }
  };

  // Fetch path history for timeline view
  const fetchPathHistory = async (path: string) => {
    setLoadingPathHistory(true);
    try {
      const response = await fetch(`/api/audit/history?path=${encodeURIComponent(path)}&days=30`);
      if (response.ok) {
        const data = await response.json();
        if (data.ok && data.reports) {
          setPathHistory(data.reports);
          setSelectedPath(path);
        }
      }
    } catch (err) {
      console.error(`[AuditDashboard] Failed to fetch history for ${path}:`, err);
    } finally {
      setLoadingPathHistory(false);
    }
  };

  // Fetch Phoenix Auditor logs
  const fetchPhoenixAuditorLogs = async () => {
    setLoadingAuditLogs(true);
    try {
      const agentsResponse = await listAgents();
      const phoenixAuditor = agentsResponse.agents.find(a => a.name === 'Phoenix Auditor');
      
      if (phoenixAuditor) {
        setPhoenixAuditorAgentId(phoenixAuditor.agent_id);
        const logsResponse = await getAgentLogs(phoenixAuditor.agent_id);
        if (logsResponse.ok) {
          const logs = logsResponse.logs || [];
          setAuditLogs(logs);
          
          // Extract paths and fetch trends
          const paths = extractPathsFromLogs(logs);
          paths.forEach(path => {
            fetchPathTrend(path);
          });
        }
      } else {
        setAuditLogs([]);
      }
    } catch (err) {
      console.error('[AuditDashboard] Failed to fetch Phoenix Auditor logs:', err);
      setAuditLogs([]);
    } finally {
      setLoadingAuditLogs(false);
    }
  };

  // Refresh audit logs when tab is active
  useEffect(() => {
    if (activeTab === 'audit-logs') {
      fetchPhoenixAuditorLogs();
      // Refresh every 10 seconds when tab is active
      const interval = setInterval(fetchPhoenixAuditorLogs, 10000);
      return () => clearInterval(interval);
    }
  }, [activeTab]);

  // Fetch draft rules from optimizer
  const fetchDraftRules = async () => {
    setLoadingDrafts(true);
    try {
      const response = await fetch('/api/phoenix/optimizer/drafts');
      if (response.ok) {
        const data: DraftRulesResponse = await response.json();
        setDraftRules(data.drafts || []);
      }
    } catch (err) {
      console.error('[AuditDashboard] Failed to fetch draft rules:', err);
    } finally {
      setLoadingDrafts(false);
    }
  };

  // Apply a draft rule
  const handleApplyRule = async (ruleId: string) => {
    if (!confirm('Are you sure you want to apply this rule? This will update the mesh configuration and propagate to all peers.')) {
      return;
    }

    setApplyingRule(ruleId);
    try {
      const response = await fetch('/api/phoenix/optimizer/apply', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rule_id: ruleId }),
      });

      const data = await response.json();
      if (data.success) {
        alert(`Rule applied successfully!\n\n${data.message}`);
        // Refresh draft rules
        await fetchDraftRules();
      } else {
        alert(`Failed to apply rule:\n\n${data.message}`);
      }
    } catch (err) {
      console.error('[AuditDashboard] Failed to apply rule:', err);
      alert('Failed to apply rule. Please try again.');
    } finally {
      setApplyingRule(null);
    }
  };

  // Fetch topic heat map and calculate decay
  useEffect(() => {
    const fetchHeatMap = async () => {
      try {
        const response = await fetch('/api/phoenix/memory/heatmap');
        if (!response.ok) throw new Error('Failed to fetch heat map');
        
        const data = await response.json();
        const topicFrequencies = data.topic_frequencies || {};
        
        // Calculate decay status for each topic
        // Since we don't have last_accessed from API, we'll estimate based on frequency
        // Topics with lower frequency are more likely to be decaying
        const now = new Date();
        const topicsList: TopicInfo[] = Object.entries(topicFrequencies).map(([topic, freq]: [string, any]) => {
          const frequency = typeof freq === 'number' ? freq : (freq?.count || 0);
          const lastAccess = freq?.last_accessed 
            ? new Date(freq.last_accessed) 
            : new Date(now.getTime() - (24 * 60 * 60 * 1000 * (1 - frequency / 100))); // Estimate
          
          const hoursSinceAccess = (now.getTime() - lastAccess.getTime()) / (1000 * 60 * 60);
          const decayPercentage = Math.min(100, (hoursSinceAccess / 24) * 100);
          
          let status: 'active' | 'decaying' | 'critical' = 'active';
          if (decayPercentage >= 90) status = 'critical';
          else if (decayPercentage >= 50) status = 'decaying';

          return {
            topic,
            frequency,
            lastAccess,
            decayPercentage,
            status,
          };
        });

        // Sort by decay percentage (highest first)
        topicsList.sort((a, b) => b.decayPercentage - a.decayPercentage);
        setTopics(topicsList);
      } catch (err) {
        console.error('[AuditDashboard] Failed to fetch heat map:', err);
      }
    };

    if (activeTab === 'memory') {
      fetchHeatMap();
      fetchSnapshotStatus();
    }
  }, [activeTab]);

  // Fetch snapshot status
  const fetchSnapshotStatus = async () => {
    try {
      const response = await fetch('/api/phoenix/memory/snapshot/status');
      if (response.ok) {
        const data = await response.json();
        setSnapshotStatus({
          hasRecent: data.has_recent_snapshot || false,
          lastSnapshotTime: data.last_snapshot_time || undefined,
        });
      }
    } catch (err) {
      console.error('[AuditDashboard] Failed to fetch snapshot status:', err);
    }
  };

  // Create snapshot
  const handleCreateSnapshot = async () => {
    setCreatingSnapshot(true);
    try {
      const response = await fetch('/api/phoenix/memory/snapshot', {
        method: 'POST',
      });

      const data = await response.json();
      if (data.success) {
        alert(`Snapshot created successfully! ${data.snapshot_paths.length} snapshot(s) saved.`);
        await fetchSnapshotStatus();
      } else {
        alert(`Failed to create snapshot: ${data.message}`);
      }
    } catch (err) {
      console.error('[AuditDashboard] Failed to create snapshot:', err);
      alert('Failed to create snapshot. Please try again.');
    } finally {
      setCreatingSnapshot(false);
    }
  };

  const handleTopicToggle = (topic: string) => {
    setSelectedTopics(prev => {
      const next = new Set(prev);
      if (next.has(topic)) {
        next.delete(topic);
      } else {
        next.add(topic);
      }
      return next;
    });
  };

  const handleSelectAll = () => {
    if (selectedTopics.size === topics.length) {
      setSelectedTopics(new Set());
    } else {
      setSelectedTopics(new Set(topics.map(t => t.topic)));
    }
  };

  const handleBatchPrune = async () => {
    if (selectedTopics.size === 0) {
      alert('Please select at least one topic to prune');
      return;
    }

    // Check for recent snapshot
    if (!snapshotStatus?.hasRecent) {
      const createSnapshot = confirm(
        'No recent snapshot found. Pruning requires a snapshot taken within the last 60 minutes.\n\n' +
        'Would you like to create a snapshot now?'
      );
      if (createSnapshot) {
        await handleCreateSnapshot();
        // Re-check status after snapshot
        await fetchSnapshotStatus();
        if (!snapshotStatus?.hasRecent) {
          alert('Snapshot created, but status not updated. Please try again in a moment.');
          return;
        }
      } else {
        return;
      }
    }

    if (!confirm(`Are you sure you want to prune ${selectedTopics.size} topic(s)? This will permanently delete all vectors related to these topics from Qdrant collections.`)) {
      return;
    }

    setPruning(true);
    const topicsArray = Array.from(selectedTopics);
    const results: { topic: string; success: boolean; deleted: number }[] = [];

    try {
      // Prune topics sequentially to avoid overwhelming the server
      for (const topic of topicsArray) {
        try {
          const response = await fetch('/api/phoenix/memory/prune', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ topic }),
          });

          const data = await response.json();
          results.push({
            topic,
            success: data.success,
            deleted: data.deleted_count || 0,
          });
        } catch (err) {
          results.push({
            topic,
            success: false,
            deleted: 0,
          });
        }
      }

      const successCount = results.filter(r => r.success).length;
      const totalDeleted = results.reduce((sum, r) => sum + r.deleted, 0);

      alert(`Pruning complete: ${successCount}/${topicsArray.length} topics pruned successfully. ${totalDeleted} vectors deleted.`);

      // Refresh topics list
      setSelectedTopics(new Set());
      const response = await fetch('/api/phoenix/memory/heatmap');
      if (response.ok) {
        const data = await response.json();
        const topicFrequencies = data.topic_frequencies || {};
        const topicsList: TopicInfo[] = Object.entries(topicFrequencies).map(([topic, freq]: [string, any]) => {
          const frequency = typeof freq === 'number' ? freq : (freq?.count || 0);
          const now = new Date();
          const lastAccess = freq?.last_accessed 
            ? new Date(freq.last_accessed) 
            : new Date(now.getTime() - (24 * 60 * 60 * 1000 * (1 - frequency / 100)));
          const hoursSinceAccess = (now.getTime() - lastAccess.getTime()) / (1000 * 60 * 60);
          const decayPercentage = Math.min(100, (hoursSinceAccess / 24) * 100);
          let status: 'active' | 'decaying' | 'critical' = 'active';
          if (decayPercentage >= 90) status = 'critical';
          else if (decayPercentage >= 50) status = 'decaying';
          return { topic, frequency, lastAccess, decayPercentage, status };
        });
        topicsList.sort((a, b) => b.decayPercentage - a.decayPercentage);
        setTopics(topicsList);
      }
    } catch (err) {
      console.error('[AuditDashboard] Batch prune error:', err);
      alert('Failed to prune topics. Please try again.');
    } finally {
      setPruning(false);
    }
  };

  // Calculate mesh trust score from conflict profiles
  const calculateMeshTrustScore = (entry: GovernanceReportEntry): number => {
    if (entry.conflict_profile.length === 0) return 100;
    
    const approvedCount = entry.conflict_profile.filter(c => c.approved).length;
    const totalCount = entry.conflict_profile.length;
    const averageScore = entry.conflict_profile.reduce((sum, c) => sum + c.compliance_score, 0) / totalCount;
    
    // Weighted score: 70% approval rate, 30% average compliance
    return (approvedCount / totalCount) * 100 * 0.7 + averageScore * 0.3;
  };

  // Simple markdown renderer
  const renderMarkdown = (markdown: string) => {
    // Split into lines and render basic markdown
    const lines = markdown.split('\n');
    const elements: React.ReactElement[] = [];
    let inCodeBlock = false;
    let codeBlockContent: string[] = [];
    let listItems: string[] = [];
    let inList = false;

    lines.forEach((line, idx) => {
      // Code blocks
      if (line.startsWith('```')) {
        if (inCodeBlock) {
          elements.push(
            <pre key={`code-${idx}`} className="bg-[rgb(var(--bg-secondary-rgb)/1)] p-4 rounded-lg overflow-x-auto my-4">
              <code className="text-sm font-mono text-[var(--text-primary)]">
                {codeBlockContent.join('\n')}
              </code>
            </pre>
          );
          codeBlockContent = [];
          inCodeBlock = false;
        } else {
          inCodeBlock = true;
        }
        return;
      }

      if (inCodeBlock) {
        codeBlockContent.push(line);
        return;
      }

      // Lists
      if (line.trim().startsWith('- ') || line.trim().startsWith('* ')) {
        if (!inList) {
          inList = true;
          listItems = [];
        }
        listItems.push(line.trim().substring(2));
        return;
      } else if (inList && line.trim() === '') {
        elements.push(
          <ul key={`list-${idx}`} className="list-disc list-inside my-2 space-y-1">
            {listItems.map((item, i) => (
              <li key={i} className="text-[var(--text-primary)]">{item}</li>
            ))}
          </ul>
        );
        listItems = [];
        inList = false;
      } else if (inList) {
        listItems.push(line.trim());
        return;
      }

      // Headers
      if (line.startsWith('# ')) {
        elements.push(<h1 key={idx} className="text-2xl font-bold text-[var(--text-primary)] mt-6 mb-3">{line.substring(2)}</h1>);
      } else if (line.startsWith('## ')) {
        elements.push(<h2 key={idx} className="text-xl font-bold text-[var(--text-primary)] mt-5 mb-2">{line.substring(3)}</h2>);
      } else if (line.startsWith('### ')) {
        elements.push(<h3 key={idx} className="text-lg font-semibold text-[var(--text-primary)] mt-4 mb-2">{line.substring(4)}</h3>);
      } else if (line.startsWith('**') && line.endsWith('**')) {
        // Bold text
        const content = line.replace(/\*\*/g, '');
        elements.push(<p key={idx} className="font-bold text-[var(--text-primary)] my-2">{content}</p>);
      } else if (line.trim() === '---') {
        elements.push(<hr key={idx} className="my-4 border-[rgb(var(--bg-steel-rgb)/0.3)]" />);
      } else if (line.trim() !== '') {
        // Regular paragraph
        const processedLine = line
          .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
          .replace(/`(.+?)`/g, '<code class="bg-[rgb(var(--bg-secondary-rgb)/1)] px-1 py-0.5 rounded text-sm font-mono">$1</code>');
        elements.push(
          <p 
            key={idx} 
            className="text-[var(--text-primary)] my-2"
            dangerouslySetInnerHTML={{ __html: processedLine }}
          />
        );
      } else {
        elements.push(<br key={idx} />);
      }
    });

    // Handle remaining list items
    if (inList && listItems.length > 0) {
      elements.push(
        <ul key="list-final" className="list-disc list-inside my-2 space-y-1">
          {listItems.map((item, i) => (
            <li key={i} className="text-[var(--text-primary)]">{item}</li>
          ))}
        </ul>
      );
    }

    // Handle remaining code block
    if (inCodeBlock && codeBlockContent.length > 0) {
      elements.push(
        <pre key="code-final" className="bg-[rgb(var(--bg-secondary-rgb)/1)] p-4 rounded-lg overflow-x-auto my-4">
          <code className="text-sm font-mono text-[var(--text-primary)]">
            {codeBlockContent.join('\n')}
          </code>
        </pre>
      );
    }

    return elements;
  };

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  // Show recovery console if requested
  if (showRecoveryConsole) {
    return (
      <PhoenixRecoveryConsole
        onClose={() => setShowRecoveryConsole(false)}
      />
    );
  }

  return (
    <div className="flex-1 flex flex-col bg-[var(--bg-primary)] overflow-hidden font-display text-[var(--text-primary)]">
      {/* Header */}
      <div className="p-6 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-[var(--bg-steel)]">assessment</span>
            <h2 className="text-xl font-bold text-[var(--text-primary)] uppercase tracking-tight">
              Phoenix Audit Dashboard
            </h2>
          </div>
          {onClose && (
            <button
              onClick={onClose}
              className="px-4 py-2 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded hover:bg-[var(--accent-hover)] transition-colors"
            >
              Close
            </button>
          )}
        </div>

        {/* Tabs */}
        <div className="flex gap-2">
          <button
            onClick={() => setActiveTab('report')}
            className={`px-4 py-2 rounded transition-colors ${
              activeTab === 'report'
                ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                : 'bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[rgb(var(--bg-steel-rgb)/0.3)]'
            }`}
          >
            Governance Report
          </button>
          <button
            onClick={() => setActiveTab('timeline')}
            className={`px-4 py-2 rounded transition-colors ${
              activeTab === 'timeline'
                ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                : 'bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[rgb(var(--bg-steel-rgb)/0.3)]'
            }`}
          >
            Override Timeline
          </button>
          <button
            onClick={() => setActiveTab('memory')}
            className={`px-4 py-2 rounded transition-colors ${
              activeTab === 'memory'
                ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                : 'bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[rgb(var(--bg-steel-rgb)/0.3)]'
            }`}
          >
            Memory Hygiene
          </button>
          <button
            onClick={() => setActiveTab('audit-logs')}
            className={`px-4 py-2 rounded transition-colors ${
              activeTab === 'audit-logs'
                ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                : 'bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[rgb(var(--bg-steel-rgb)/0.3)]'
            }`}
          >
            Audit Logs
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {loading && (
          <div className="text-center py-12 text-[var(--bg-steel)]">
            <span className="material-symbols-outlined text-4xl mb-2 animate-spin">hourglass_empty</span>
            <p>Loading audit data...</p>
          </div>
        )}

        {error && (
          <div className="p-4 bg-[rgb(var(--danger-rgb)/0.1)] border border-[rgb(var(--danger-rgb)/0.3)] text-[var(--text-primary)] rounded mb-4">
            <strong>Error:</strong> {error}
          </div>
        )}

        {!loading && !error && (
          <>
            {/* Rule Suggestions Banner */}
            {draftRules.length > 0 && (
              <div className="mb-4 p-4 bg-[rgb(var(--warning-rgb)/0.1)] border border-[rgb(var(--warning-rgb)/0.3)] rounded-lg">
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-[rgb(var(--warning-rgb)/0.9)]">
                      auto_fix_high
                    </span>
                    <div>
                      <div className="text-sm font-semibold text-[var(--text-primary)]">
                        Rule Suggestions Available
                      </div>
                      <div className="text-xs text-[var(--bg-steel)]">
                        {draftRules.length} draft rule(s) generated from strategic recommendations
                      </div>
                    </div>
                  </div>
                </div>
                <div className="space-y-2">
                  {draftRules.map((rule) => (
                    <div
                      key={rule.id}
                      className="bg-[rgb(var(--bg-secondary-rgb)/1)] rounded p-3 border border-[rgb(var(--bg-steel-rgb)/0.2)]"
                    >
                      <div className="flex items-start justify-between mb-2">
                        <div className="flex-1">
                          <div className="text-sm font-semibold text-[var(--text-primary)] mb-1">
                            {rule.description}
                          </div>
                          <div className="text-xs text-[var(--bg-steel)] mb-2">
                            Confidence: {(rule.confidence * 100).toFixed(1)}% | 
                            Based on {rule.source_recommendations.length} recommendation(s)
                          </div>
                          <div className="text-xs text-[var(--text-primary)] bg-[rgb(var(--bg-primary-rgb)/0.5)] p-2 rounded font-mono whitespace-pre-wrap">
                            {rule.proposed_change.substring(0, 200)}
                            {rule.proposed_change.length > 200 ? '...' : ''}
                          </div>
                        </div>
                        <button
                          onClick={() => handleApplyRule(rule.id)}
                          disabled={applyingRule === rule.id || loadingDrafts}
                          className="ml-3 px-3 py-1.5 bg-[rgb(var(--warning-rgb)/1)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--warning-rgb)/0.9)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm flex items-center gap-1"
                        >
                          <span className="material-symbols-outlined text-sm">
                            {applyingRule === rule.id ? 'hourglass_empty' : 'check_circle'}
                          </span>
                          {applyingRule === rule.id ? 'Applying...' : 'Apply'}
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Report Viewer */}
            {activeTab === 'report' && (
              <div className="bg-[rgb(var(--surface-rgb)/1)] rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.3)]">
                {reportMarkdown ? (
                  <div className="prose prose-invert max-w-none">
                    {renderMarkdown(reportMarkdown)}
                  </div>
                ) : (
                  <p className="text-[var(--bg-steel)]">No governance report available.</p>
                )}
              </div>
            )}

            {/* Override Timeline */}
            {activeTab === 'timeline' && (
              <div className="space-y-4">
                {reportJson && reportJson.entries.length > 0 ? (
                  reportJson.entries.map((entry, idx) => {
                    const trustScore = calculateMeshTrustScore(entry);
                    return (
                      <div
                        key={idx}
                        className="bg-[rgb(var(--surface-rgb)/1)] rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.3)]"
                      >
                        <div className="flex items-start justify-between mb-4">
                          <div>
                            <h3 className="text-lg font-bold text-[var(--text-primary)] mb-2">
                              Override #{idx + 1} - {entry.agent_id}
                            </h3>
                            <p className="text-sm text-[var(--bg-steel)]">
                              {formatTimestamp(entry.override_timestamp)}
                            </p>
                          </div>
                          <div className="text-right">
                            <div className="text-sm text-[var(--bg-steel)] mb-1">Mesh Trust Score</div>
                            <div className={`text-2xl font-bold ${
                              trustScore >= 80 ? 'text-[var(--success)]' :
                              trustScore >= 60 ? 'text-[var(--warning)]' :
                              'text-[var(--danger)]'
                            }`}>
                              {trustScore.toFixed(1)}%
                            </div>
                          </div>
                        </div>

                        <div className="mb-4">
                          <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2">Rationale:</h4>
                          <p className="text-[var(--text-primary)] bg-[rgb(var(--bg-secondary-rgb)/1)] p-3 rounded">
                            {entry.rationale}
                          </p>
                        </div>

                        <div className="mb-4">
                          <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2">Commit Hash:</h4>
                          <code className="text-xs font-mono bg-[rgb(var(--bg-secondary-rgb)/1)] px-2 py-1 rounded">
                            {entry.commit_hash}
                          </code>
                        </div>

                        {entry.conflict_profile.length > 0 && (
                          <div className="mb-4">
                            <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2">Voting Nodes:</h4>
                            <div className="space-y-2">
                              {entry.conflict_profile.map((conflict, cIdx) => (
                                <div
                                  key={cIdx}
                                  className="flex items-center justify-between p-2 bg-[rgb(var(--bg-secondary-rgb)/1)] rounded"
                                >
                                  <div className="flex items-center gap-2">
                                    <span className={`text-sm ${conflict.approved ? 'text-[var(--success)]' : 'text-[var(--danger)]'}`}>
                                      {conflict.approved ? '✅' : '❌'}
                                    </span>
                                    <code className="text-xs font-mono text-[var(--text-primary)]">
                                      {conflict.node_id}
                                    </code>
                                  </div>
                                  <div className="text-sm text-[var(--bg-steel)]">
                                    Score: {conflict.compliance_score.toFixed(1)}%
                                  </div>
                                </div>
                              ))}
                            </div>
                          </div>
                        )}

                        <div className="text-sm text-[var(--bg-steel)]">
                          <p><strong>Impact:</strong> {entry.impact_summary}</p>
                          <p className="mt-1">
                            <strong>Knowledge Fragments Since:</strong> {entry.knowledge_fragments_since.toLocaleString()}
                          </p>
                        </div>
                      </div>
                    );
                  })
                ) : (
                  <div className="text-center py-12 text-[var(--bg-steel)]">
                    <span className="material-symbols-outlined text-4xl mb-2">inbox</span>
                    <p>No strategic overrides recorded.</p>
                  </div>
                )}
              </div>
            )}

            {/* Memory Hygiene */}
            {activeTab === 'memory' && (
              <div className="space-y-4">
                {/* Snapshot Status Banner */}
                {snapshotStatus && (
                  <div className={`p-4 rounded-lg border ${
                    snapshotStatus.hasRecent
                      ? 'bg-[rgb(var(--success-rgb)/0.1)] border-[rgb(var(--success-rgb)/0.3)]'
                      : 'bg-[rgb(var(--warning-rgb)/0.1)] border-[rgb(var(--warning-rgb)/0.3)]'
                  }`}>
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <span className={`material-symbols-outlined ${
                          snapshotStatus.hasRecent ? 'text-[rgb(var(--success-rgb)/0.9)]' : 'text-[rgb(var(--warning-rgb)/0.95)]'
                        }`}>
                          {snapshotStatus.hasRecent ? 'check_circle' : 'warning'}
                        </span>
                        <div>
                          <div className="text-sm font-semibold text-[var(--text-primary)]">
                            {snapshotStatus.hasRecent
                              ? 'Recent Snapshot Available'
                              : 'No Recent Snapshot'}
                          </div>
                          {snapshotStatus.lastSnapshotTime && (
                            <div className="text-xs text-[var(--bg-steel)]">
                              Last snapshot: {formatTimestamp(snapshotStatus.lastSnapshotTime)}
                            </div>
                          )}
                        </div>
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => setShowRecoveryConsole(true)}
                          className="px-4 py-2 bg-[rgb(var(--warning-rgb)/1)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--warning-rgb)/0.9)] transition-colors text-sm flex items-center gap-2"
                          title="Point-in-Time Recovery - Restore from previous snapshots"
                        >
                          <span className="material-symbols-outlined text-sm">restore</span>
                          Point-in-Time Recovery
                        </button>
                        <button
                          onClick={handleCreateSnapshot}
                          disabled={creatingSnapshot}
                          className="px-4 py-2 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm flex items-center gap-2"
                        >
                          <span className="material-symbols-outlined text-sm">
                            {creatingSnapshot ? 'hourglass_empty' : 'camera_enhance'}
                          </span>
                          {creatingSnapshot ? 'Creating...' : 'Take Mesh Snapshot'}
                        </button>
                      </div>
                    </div>
                  </div>
                )}

                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-bold text-[var(--text-primary)]">Knowledge Inventory</h3>
                  <div className="flex gap-2">
                    <button
                      onClick={handleSelectAll}
                      className="px-4 py-2 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] transition-colors text-sm"
                    >
                      {selectedTopics.size === topics.length ? 'Deselect All' : 'Select All'}
                    </button>
                    <button
                      onClick={handleBatchPrune}
                      disabled={selectedTopics.size === 0 || pruning || !snapshotStatus?.hasRecent}
                      className="px-4 py-2 bg-[rgb(var(--danger-rgb)/1)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--danger-rgb)/0.9)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                      title={!snapshotStatus?.hasRecent ? 'A recent snapshot is required before pruning' : ''}
                    >
                      {pruning ? 'Pruning...' : `Prune Selected (${selectedTopics.size})`}
                    </button>
                  </div>
                </div>

                {topics.length > 0 ? (
                  <div className="bg-[rgb(var(--surface-rgb)/1)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] overflow-hidden">
                    <table className="w-full">
                      <thead className="bg-[var(--bg-secondary)] border-b border-[rgb(var(--bg-steel-rgb)/0.3)]">
                        <tr>
                          <th className="px-4 py-3 text-left text-sm font-semibold text-[var(--text-primary)] uppercase tracking-tight w-12">
                            <input
                              type="checkbox"
                              checked={selectedTopics.size === topics.length && topics.length > 0}
                              onChange={handleSelectAll}
                              className="rounded"
                            />
                          </th>
                          <th className="px-4 py-3 text-left text-sm font-semibold text-[var(--text-primary)] uppercase tracking-tight">
                            Topic
                          </th>
                          <th className="px-4 py-3 text-left text-sm font-semibold text-[var(--text-primary)] uppercase tracking-tight">
                            Frequency
                          </th>
                          <th className="px-4 py-3 text-left text-sm font-semibold text-[var(--text-primary)] uppercase tracking-tight">
                            Last Access
                          </th>
                          <th className="px-4 py-3 text-left text-sm font-semibold text-[var(--text-primary)] uppercase tracking-tight">
                            Decay Status
                          </th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-[rgb(var(--bg-steel-rgb)/0.1)]">
                        {topics.map((topic) => (
                          <tr
                            key={topic.topic}
                            className={`hover:bg-[rgb(var(--bg-primary-rgb)/0.2)] transition-colors ${
                              topic.status === 'critical' ? 'bg-[rgb(var(--danger-rgb)/0.05)]' :
                              topic.status === 'decaying' ? 'bg-[rgb(var(--warning-rgb)/0.05)]' : ''
                            }`}
                          >
                            <td className="px-4 py-3">
                              <input
                                type="checkbox"
                                checked={selectedTopics.has(topic.topic)}
                                onChange={() => handleTopicToggle(topic.topic)}
                                className="rounded"
                              />
                            </td>
                            <td className="px-4 py-3 text-sm text-[var(--text-primary)] font-mono">
                              {topic.topic}
                            </td>
                            <td className="px-4 py-3 text-sm text-[var(--text-primary)]">
                              {topic.frequency.toLocaleString()}
                            </td>
                            <td className="px-4 py-3 text-sm text-[var(--bg-steel)]">
                              {topic.lastAccess ? formatTimestamp(topic.lastAccess.toISOString()) : 'Unknown'}
                            </td>
                            <td className="px-4 py-3">
                              <div className="flex items-center gap-2">
                                <div className="flex-1 bg-[rgb(var(--bg-secondary-rgb)/1)] rounded-full h-2 overflow-hidden">
                                  <div
                                    className={`h-full transition-all ${
                                      topic.status === 'critical' ? 'bg-[var(--danger)]' :
                                      topic.status === 'decaying' ? 'bg-[var(--warning)]' :
                                      'bg-[var(--success)]'
                                    }`}
                                    style={{ width: `${topic.decayPercentage}%` }}
                                  />
                                </div>
                                <span className={`text-xs font-semibold min-w-[60px] text-right ${
                                  topic.status === 'critical' ? 'text-[var(--danger)]' :
                                  topic.status === 'decaying' ? 'text-[var(--warning)]' :
                                  'text-[var(--success)]'
                                }`}>
                                  {topic.decayPercentage.toFixed(1)}%
                                </span>
                              </div>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  <div className="text-center py-12 text-[var(--bg-steel)]">
                    <span className="material-symbols-outlined text-4xl mb-2">inbox</span>
                    <p>No topics found in knowledge inventory.</p>
                  </div>
                )}
              </div>
            )}

            {/* Audit Logs Tab */}
            {activeTab === 'audit-logs' && (
              <div className="space-y-4">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-bold text-[var(--text-primary)]">Phoenix Auditor Activity</h3>
                  <button
                    onClick={fetchPhoenixAuditorLogs}
                    disabled={loadingAuditLogs}
                    className="px-4 py-2 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm flex items-center gap-2"
                  >
                    <span className="material-symbols-outlined text-sm">
                      {loadingAuditLogs ? 'hourglass_empty' : 'refresh'}
                    </span>
                    {loadingAuditLogs ? 'Loading...' : 'Refresh'}
                  </button>
                </div>

                {loadingAuditLogs && auditLogs.length === 0 ? (
                  <div className="text-center py-12 text-[var(--bg-steel)]">
                    <span className="material-symbols-outlined text-4xl mb-2 animate-spin">hourglass_empty</span>
                    <p>Loading audit logs...</p>
                  </div>
                ) : !phoenixAuditorAgentId ? (
                  <div className="text-center py-12 text-[var(--bg-steel)]">
                    <span className="material-symbols-outlined text-4xl mb-2">info</span>
                    <p>Phoenix Auditor agent not found. It may not be initialized yet.</p>
                  </div>
                ) : auditLogs.length === 0 ? (
                  <div className="text-center py-12 text-[var(--bg-steel)]">
                    <span className="material-symbols-outlined text-4xl mb-2">inbox</span>
                    <p>No audit logs available yet.</p>
                  </div>
                ) : (
                  <div className="space-y-4">
                    {selectedPath && (
                      <div className="bg-[rgb(var(--surface-rgb)/1)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] p-4">
                        <div className="flex items-center justify-between mb-4">
                          <h4 className="text-md font-semibold text-[var(--text-primary)]">
                            Drift Timeline: <span className="font-mono text-sm">{selectedPath}</span>
                          </h4>
                          <button
                            onClick={() => {
                              setSelectedPath(null);
                              setPathHistory([]);
                            }}
                            className="px-3 py-1 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] transition-colors text-sm"
                          >
                            Close
                          </button>
                        </div>
                        {loadingPathHistory ? (
                          <div className="text-center py-8 text-[var(--bg-steel)]">
                            <span className="material-symbols-outlined text-2xl mb-2 animate-spin">hourglass_empty</span>
                            <p>Loading history...</p>
                          </div>
                        ) : pathHistory.length > 0 ? (
                          <div className="space-y-2 max-h-[400px] overflow-y-auto">
                            {pathHistory.map((report, idx) => (
                              <div
                                key={report.id || idx}
                                className="p-3 bg-[rgb(var(--bg-secondary-rgb)/1)] rounded border border-[rgb(var(--bg-steel-rgb)/0.2)]"
                              >
                                <div className="flex items-start justify-between mb-2">
                                  <div className="text-xs text-[var(--bg-steel)]">
                                    {new Date(report.timestamp).toLocaleString()}
                                  </div>
                                  <span className={`px-2 py-1 rounded text-xs font-semibold ${
                                    report.severity === 'CRITICAL' ? 'bg-[var(--danger)] text-white' :
                                    report.severity === 'HIGH' ? 'bg-[var(--warning)] text-white' :
                                    report.severity === 'MEDIUM' ? 'bg-[rgb(var(--warning-rgb)/0.5)] text-[var(--text-primary)]' :
                                    'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                                  }`}>
                                    {report.severity}
                                  </span>
                                </div>
                                <div className="text-sm text-[var(--text-primary)] mb-1">
                                  <strong>Climax:</strong> {report.report?.climax || 'N/A'}
                                </div>
                                {report.report?.executive_pulse && (
                                  <div className="text-xs text-[var(--bg-steel)] italic">
                                    {report.report.executive_pulse}
                                  </div>
                                )}
                              </div>
                            ))}
                          </div>
                        ) : (
                          <div className="text-center py-8 text-[var(--bg-steel)]">
                            <p>No history found for this path.</p>
                          </div>
                        )}
                      </div>
                    )}
                    <div className="bg-[rgb(var(--surface-rgb)/1)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] overflow-hidden">
                      <div className="max-h-[600px] overflow-y-auto">
                        <div className="p-4 space-y-3">
                          {auditLogs.map((log, idx) => {
                            // Extract paths from this log entry
                            const logPaths = extractPathsFromLogs([log]);
                            
                            return (
                              <div
                                key={idx}
                                className="p-3 bg-[rgb(var(--bg-secondary-rgb)/1)] rounded border border-[rgb(var(--bg-steel-rgb)/0.2)] font-mono text-sm text-[var(--text-primary)] whitespace-pre-wrap break-words"
                              >
                                <div className="flex flex-wrap gap-2 mb-2">
                                  {logPaths.map((path, pathIdx) => {
                                    const trend = pathTrends.get(path);
                                    return (
                                      <button
                                        key={pathIdx}
                                        onClick={() => fetchPathHistory(path)}
                                        className="group relative px-2 py-1 bg-[rgb(var(--bg-primary-rgb)/0.3)] hover:bg-[rgb(var(--bg-primary-rgb)/0.5)] rounded text-xs font-mono text-[var(--text-primary)] transition-colors flex items-center gap-1"
                                      >
                                        <span>{path}</span>
                                        {trend && trend.changeCount > 0 && (
                                          <span
                                            className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                                              trend.recurring || trend.severityEscalation
                                                ? 'bg-[var(--danger)] text-white'
                                                : trend.changeCount >= 3
                                                ? 'bg-[var(--warning)] text-white'
                                                : 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                                            }`}
                                            title={`${trend.changeCount} change(s) in last 30 days${trend.recurring ? ' - Recurring issue' : ''}${trend.severityEscalation ? ' - Severity escalating' : ''}`}
                                          >
                                            {trend.changeCount}
                                            {trend.recurring && '⚠️'}
                                          </span>
                                        )}
                                      </button>
                                    );
                                  })}
                                </div>
                                <div>{log}</div>
                              </div>
                            );
                          })}
                        </div>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default AuditDashboard;
