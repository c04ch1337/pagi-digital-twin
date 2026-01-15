import React, { useState, useEffect } from 'react';
import { getAllPlaybooks, searchPlaybooksByQuery, Playbook, deployPlaybookToCluster } from '../services/playbookService';

interface PlaybookLibraryProps {
  className?: string;
}

const PlaybookLibrary: React.FC<PlaybookLibraryProps> = ({ className = '' }) => {
  const [playbooks, setPlaybooks] = useState<Playbook[]>([]);
  const [filteredPlaybooks, setFilteredPlaybooks] = useState<Playbook[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [minReliability, setMinReliability] = useState<number>(0.7);
  const [sortBy, setSortBy] = useState<'reliability' | 'recent' | 'name'>('reliability');
  const [deploying, setDeploying] = useState<Set<string>>(new Set());

  useEffect(() => {
    const fetchPlaybooks = async () => {
      try {
        setLoading(true);
        setError(null);
        const allPlaybooks = await getAllPlaybooks();
        setPlaybooks(allPlaybooks);
        setFilteredPlaybooks(allPlaybooks);
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to fetch playbooks';
        setError(msg);
        console.error('[PlaybookLibrary] Failed to fetch playbooks:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchPlaybooks();
    // Refresh every 60 seconds
    const interval = setInterval(fetchPlaybooks, 60000);
    return () => clearInterval(interval);
  }, []);

  // Filter and sort playbooks
  useEffect(() => {
    let filtered = [...playbooks];

    // Apply reliability filter
    filtered = filtered.filter(pb => pb.reliability_score >= minReliability);

    // Apply search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(pb => 
        pb.tool_name.toLowerCase().includes(query) ||
        pb.installation_command.toLowerCase().includes(query) ||
        pb.description?.toLowerCase().includes(query) ||
        pb.language?.toLowerCase().includes(query)
      );
    }

    // Sort
    filtered.sort((a, b) => {
      switch (sortBy) {
        case 'reliability':
          return b.reliability_score - a.reliability_score;
        case 'recent':
          const aTime = new Date(a.last_used_at || a.verified_at).getTime();
          const bTime = new Date(b.last_used_at || b.verified_at).getTime();
          return bTime - aTime;
        case 'name':
          return a.tool_name.localeCompare(b.tool_name);
        default:
          return 0;
      }
    });

    setFilteredPlaybooks(filtered);
  }, [playbooks, searchQuery, minReliability, sortBy]);

  const formatDate = (dateString: string) => {
    try {
      return new Date(dateString).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
      });
    } catch {
      return dateString;
    }
  };

  const getReliabilityColor = (score: number): string => {
    if (score >= 0.9) return 'text-[var(--success)]';
    if (score >= 0.7) return 'text-[var(--bg-steel)]';
    if (score >= 0.5) return 'text-[rgb(var(--warning-rgb))]';
    return 'text-[rgb(var(--danger-rgb))]';
  };

  const handleDeployToFleet = async (playbook: Playbook) => {
    if (!confirm(`Deploy "${playbook.tool_name}" to all agent stations in the cluster?`)) {
      return;
    }

    setDeploying(prev => new Set(prev).add(playbook.id));

    try {
      const result = await deployPlaybookToCluster(playbook.id);
      if (result.ok && result.deployment) {
        alert(
          `Deployment initiated!\n\n` +
          `Total Agents: ${result.deployment.total_agents}\n` +
          `Successful: ${result.deployment.successful_deployments}\n` +
          `Failed: ${result.deployment.failed_deployments}`
        );
      } else {
        alert(`Deployment failed: ${result.error || 'Unknown error'}`);
      }
    } catch (error) {
      console.error('[PlaybookLibrary] Deployment error:', error);
      alert(`Deployment failed: ${error instanceof Error ? error.message : 'Unknown error'}`);
    } finally {
      setDeploying(prev => {
        const next = new Set(prev);
        next.delete(playbook.id);
        return next;
      });
    }
  };

  if (loading && playbooks.length === 0) {
    return (
      <div className={`${className} flex items-center justify-center h-full`}>
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">
          Loading playbook library...
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

  return (
    <div className={`${className} h-full flex flex-col overflow-hidden`}>
      <div className="mb-3">
        <h4 className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
          Playbook Library
        </h4>
        <p className="text-[9px] text-[var(--text-secondary)] opacity-70 mb-3">
          Verified tool installation playbooks from the agent cluster
        </p>

        {/* Search and Filters */}
        <div className="space-y-2 mb-3">
          <input
            type="text"
            placeholder="Search playbooks..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-2 py-1.5 text-[10px] bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-[var(--text-primary)] placeholder-[var(--text-secondary)] focus:outline-none focus:border-[var(--bg-steel)]"
          />
          
          <div className="flex items-center gap-3">
            <label className="text-[9px] text-[var(--text-secondary)] flex items-center gap-1.5">
              <span>Min Reliability:</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.1"
                value={minReliability}
                onChange={(e) => setMinReliability(parseFloat(e.target.value))}
                className="flex-1"
              />
              <span className="w-8 text-right">{(minReliability * 100).toFixed(0)}%</span>
            </label>

            <select
              value={sortBy}
              onChange={(e) => setSortBy(e.target.value as 'reliability' | 'recent' | 'name')}
              className="px-2 py-1 text-[9px] bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded text-[var(--text-primary)] focus:outline-none focus:border-[var(--bg-steel)]"
            >
              <option value="reliability">Sort by Reliability</option>
              <option value="recent">Sort by Recent</option>
              <option value="name">Sort by Name</option>
            </select>
          </div>
        </div>
      </div>

      {/* Playbook List */}
      <div className="flex-1 overflow-auto">
        {filteredPlaybooks.length === 0 ? (
          <div className="text-center py-8">
            <div className="text-[10px] text-[var(--text-secondary)] opacity-70">
              {playbooks.length === 0 
                ? 'No playbooks available yet' 
                : 'No playbooks match your filters'}
            </div>
          </div>
        ) : (
          <div className="space-y-2">
            {filteredPlaybooks.map((playbook) => (
              <div
                key={playbook.id}
                className="p-3 bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.2)] rounded-lg hover:bg-[rgb(var(--surface-rgb)/0.8)] transition-colors"
              >
                <div className="flex items-start justify-between mb-2">
                  <div className="flex-1">
                    <h5 className="text-[11px] font-semibold text-[var(--text-primary)] mb-1">
                      {playbook.tool_name}
                    </h5>
                    {playbook.description && (
                      <p className="text-[9px] text-[var(--text-secondary)] opacity-80 mb-1.5 line-clamp-2">
                        {playbook.description}
                      </p>
                    )}
                  </div>
                  <div className={`text-[10px] font-bold ${getReliabilityColor(playbook.reliability_score)} ml-2`}>
                    {(playbook.reliability_score * 100).toFixed(0)}%
                  </div>
                </div>

                <div className="space-y-1 text-[9px] text-[var(--text-secondary)]">
                  <div className="flex items-center gap-2">
                    <span className="opacity-70">Command:</span>
                    <code className="px-1.5 py-0.5 bg-[rgb(var(--bg-secondary-rgb)/0.3)] rounded text-[8px] font-mono">
                      {playbook.installation_command}
                    </code>
                  </div>

                  <div className="flex items-center gap-4 flex-wrap">
                    {playbook.language && (
                      <span>
                        <span className="opacity-70">Language:</span> {playbook.language}
                      </span>
                    )}
                    {playbook.verified_by_agent && (
                      <span>
                        <span className="opacity-70">Verified by:</span> {playbook.verified_by_agent}
                      </span>
                    )}
                    <span>
                      <span className="opacity-70">Success:</span> {playbook.success_count}/{playbook.total_attempts}
                    </span>
                  </div>

                  <div className="flex items-center gap-4 text-[8px] opacity-60">
                    <span>Verified: {formatDate(playbook.verified_at)}</span>
                    {playbook.last_used_at && (
                      <span>Last used: {formatDate(playbook.last_used_at)}</span>
                    )}
                  </div>

                  {/* Promote to Fleet Button */}
                  {playbook.reliability_score >= 0.9 && (
                    <div className="mt-2 pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                      <button
                        onClick={() => handleDeployToFleet(playbook)}
                        disabled={deploying.has(playbook.id)}
                        className={`w-full px-2 py-1.5 text-[9px] font-semibold rounded transition-all ${
                          deploying.has(playbook.id)
                            ? 'bg-[rgb(var(--bg-secondary-rgb)/0.5)] text-[var(--text-secondary)] cursor-not-allowed'
                            : 'bg-[var(--success)]/20 hover:bg-[var(--success)]/30 text-[var(--success)] border border-[var(--success)]/40 hover:border-[var(--success)]/60'
                        }`}
                        title="Deploy this playbook to all agent stations in the cluster"
                      >
                        {deploying.has(playbook.id) ? (
                          <span className="flex items-center justify-center gap-1">
                            <span className="material-symbols-outlined text-[12px] animate-spin">sync</span>
                            Deploying...
                          </span>
                        ) : (
                          <span className="flex items-center justify-center gap-1">
                            <span className="material-symbols-outlined text-[12px]">rocket_launch</span>
                            Promote to Fleet
                          </span>
                        )}
                      </button>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Footer Stats */}
      <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
        <div className="text-[9px] text-[var(--text-secondary)] opacity-70">
          Showing {filteredPlaybooks.length} of {playbooks.length} playbooks
        </div>
      </div>
    </div>
  );
};

export default PlaybookLibrary;
