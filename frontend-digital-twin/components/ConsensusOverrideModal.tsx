import React, { useState, useEffect, useRef } from 'react';
import { Shield, CheckCircle, XCircle, AlertTriangle, GitBranch, User } from 'lucide-react';

interface VoteDetail {
  node_id: string;
  compliance_score: number;
  approved: boolean;
  timestamp: string;
}

interface ConsensusOverrideModalProps {
  commitHash: string;
  agentName: string;
  onClose: () => void;
  onOverride: (rationale: string) => Promise<void>;
}

const ConsensusOverrideModal: React.FC<ConsensusOverrideModalProps> = ({
  commitHash,
  agentName,
  onClose,
  onOverride,
}) => {
  const [votes, setVotes] = useState<VoteDetail[]>([]);
  const [loading, setLoading] = useState(true);
  const [overrideRationale, setOverrideRationale] = useState('');
  const [overriding, setOverriding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    fetchVoteDetails();
  }, [commitHash]);

  // Handle Esc key with confirmation
  useEffect(() => {
    const handleEscKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !overriding) {
        if (overrideRationale.trim()) {
          if (confirm('Cancel Override? Your rationale text will be lost.')) {
            onClose();
          }
        } else {
          onClose();
        }
      }
    };

    window.addEventListener('keydown', handleEscKey);
    return () => window.removeEventListener('keydown', handleEscKey);
  }, [overrideRationale, overriding, onClose]);

  // Lock background interactions
  useEffect(() => {
    // Apply pointer-events: none to background elements
    const appContainer = document.querySelector('[data-app-container]') || document.body;
    const originalPointerEvents = (appContainer as HTMLElement).style.pointerEvents;
    (appContainer as HTMLElement).style.pointerEvents = 'none';
    
    // Ensure modal itself is interactive
    if (modalRef.current) {
      modalRef.current.style.pointerEvents = 'auto';
    }

    return () => {
      (appContainer as HTMLElement).style.pointerEvents = originalPointerEvents || '';
    };
  }, []);

  const fetchVoteDetails = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch(`/api/consensus/votes/${commitHash}`);
      if (!response.ok) {
        throw new Error('Failed to fetch vote details');
      }
      const data = await response.json();
      setVotes(data.votes || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load vote details');
      console.error('Failed to fetch vote details:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleOverride = async () => {
    if (!overrideRationale.trim()) {
      setError('Please provide a rationale for the override');
      return;
    }

    setOverriding(true);
    setError(null);
    try {
      await onOverride(overrideRationale);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to execute override');
      console.error('Override failed:', err);
    } finally {
      setOverriding(false);
    }
  };

  const approvedVotes = votes.filter(v => v.approved).length;
  const totalVotes = votes.length;
  const averageScore = votes.length > 0
    ? votes.reduce((sum, v) => sum + v.compliance_score, 0) / votes.length
    : 0;
  const approvalPercentage = totalVotes > 0 ? (approvedVotes / totalVotes) * 100 : 0;

  return (
    <div 
      className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-[rgb(var(--bg-primary-rgb)/0.95)] backdrop-blur-md animate-in fade-in duration-200"
      data-phoenix-modal="true"
    >
      <div 
        ref={modalRef}
        className="w-full max-w-3xl bg-[rgb(var(--surface-rgb)/0.9)] border-2 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]"
        style={{
          borderColor: 'rgb(var(--bg-steel-rgb))',
          boxShadow: '0 0 30px rgba(var(--bg-steel-rgb), 0.6), 0 0 60px rgba(var(--bg-steel-rgb), 0.3), inset 0 0 20px rgba(var(--bg-steel-rgb), 0.1)',
          animation: 'phoenix-glow 3s ease-in-out infinite',
        }}
        data-phoenix-modal="true"
      >
        {/* Header */}
        <div className="p-6 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center justify-between bg-[var(--bg-secondary)]">
          <div className="flex items-center gap-3">
            <Shield className="w-5 h-5 text-[var(--bg-steel)]" />
            <div>
              <h2 className="text-xl font-bold text-[var(--text-primary)] font-display">
                Consensus Override
              </h2>
              <p className="text-sm text-[var(--text-secondary)] mt-1">
                {agentName} • Commit: <code className="font-mono text-xs">{commitHash.substring(0, 8)}</code>
              </p>
            </div>
          </div>
          <button
            onClick={() => {
              if (overrideRationale.trim()) {
                if (confirm('Cancel Override? Your rationale text will be lost.')) {
                  onClose();
                }
              } else {
                onClose();
              }
            }}
            className="text-[var(--text-secondary)] hover:text-[var(--bg-steel)] transition-colors"
            disabled={overriding}
          >
            <XCircle className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6 bg-[rgb(var(--surface-rgb)/0.6)]">
          {/* Summary Stats */}
          <div className="grid grid-cols-3 gap-4">
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4">
              <div className="text-xs text-[var(--text-secondary)] uppercase tracking-widest mb-1">
                Average Score
              </div>
              <div className="text-2xl font-bold text-[var(--text-primary)]">
                {averageScore.toFixed(1)}%
              </div>
            </div>
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4">
              <div className="text-xs text-[var(--text-secondary)] uppercase tracking-widest mb-1">
                Approval Rate
              </div>
              <div className="text-2xl font-bold text-[var(--text-primary)]">
                {approvalPercentage.toFixed(1)}%
              </div>
            </div>
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4">
              <div className="text-xs text-[var(--text-secondary)] uppercase tracking-widest mb-1">
                Total Votes
              </div>
              <div className="text-2xl font-bold text-[var(--text-primary)]">
                {totalVotes}
              </div>
            </div>
          </div>

          {/* Voting Details */}
          <div>
            <h3 className="text-sm font-bold text-[var(--text-primary)] mb-3 flex items-center gap-2">
              <User className="w-4 h-4" />
              Node Compliance Scores
            </h3>
            {loading ? (
              <div className="text-center py-8 text-[var(--text-secondary)]">
                Loading vote details...
              </div>
            ) : error ? (
              <div className="bg-[rgb(var(--danger-rgb)/0.2)] border border-[rgb(var(--danger-rgb)/0.6)] rounded-lg p-4 text-[rgb(var(--danger-rgb)/0.65)]">
                <AlertTriangle className="w-4 h-4 inline mr-2" />
                {error}
              </div>
            ) : votes.length === 0 ? (
              <div className="text-center py-8 text-[var(--text-secondary)]">
                No votes recorded yet
              </div>
            ) : (
              <div className="space-y-2 max-h-64 overflow-y-auto">
                {votes.map((vote, index) => (
                  <div
                    key={index}
                    className={`bg-[rgb(var(--surface-rgb)/0.8)] border rounded-lg p-3 flex items-center justify-between ${
                      vote.approved
                        ? 'border-[rgb(var(--success-rgb)/0.35)] bg-[rgb(var(--success-rgb)/0.08)]'
                        : 'border-[rgb(var(--danger-rgb)/0.35)] bg-[rgb(var(--danger-rgb)/0.08)]'
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      {vote.approved ? (
                        <CheckCircle className="w-4 h-4 text-[var(--success)]" />
                      ) : (
                        <XCircle className="w-4 h-4 text-[rgb(var(--danger-rgb)/0.9)]" />
                      )}
                      <div>
                        <div className="font-mono text-sm text-[var(--text-primary)]">
                          {vote.node_id.substring(0, 12)}
                        </div>
                        <div className="text-xs text-[var(--text-secondary)]">
                          {new Date(vote.timestamp).toLocaleString()}
                        </div>
                      </div>
                    </div>
                    <div className="text-right">
                      <div className="text-lg font-bold text-[var(--text-primary)]">
                        {vote.compliance_score.toFixed(1)}%
                      </div>
                      <div className="text-xs text-[var(--text-secondary)]">
                        {vote.approved ? 'Approved' : 'Rejected'}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Override Rationale */}
          <div>
            <h3 className="text-sm font-bold text-[var(--text-primary)] mb-3 flex items-center gap-2">
              <GitBranch className="w-4 h-4" />
              Strategic Override Rationale
            </h3>
            <textarea
              value={overrideRationale}
              onChange={(e) => setOverrideRationale(e.target.value)}
              placeholder="Explain why this override is necessary (e.g., 'Strategic alignment with Visionary Architect persona despite lower compliance scores')..."
              className="w-full bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4 text-[var(--text-primary)] font-mono text-sm focus:border-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)] outline-none transition-all resize-none min-h-[120px]"
            />
            <p className="text-xs text-[var(--text-secondary)] mt-2">
              This rationale will be recorded in a <code className="font-mono">[PHOENIX-OVERRIDE]</code> git commit message.
            </p>
          </div>

          {/* Warning */}
          <div className="bg-[rgb(var(--warning-rgb)/0.2)] border border-[rgb(var(--warning-rgb)/0.6)] rounded-lg p-4">
            <div className="flex items-start gap-3">
              <AlertTriangle className="w-4 h-4 text-[var(--warning)] mt-0.5" />
              <div className="text-xs text-[rgb(var(--warning-rgb)/0.7)]">
                <strong>Warning:</strong> Strategic Override will bypass the 70% compliance threshold
                and force this commit to be 'Blessed' mesh-wide. This action is logged and auditable.
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)] flex justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            disabled={overriding}
            className="px-6 py-2 rounded-lg text-sm font-bold text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-muted)] transition-all disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleOverride}
            disabled={overriding || !overrideRationale.trim()}
            className="bg-[var(--bg-steel)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] text-[var(--text-on-accent)] px-8 py-2 rounded-lg text-sm font-bold shadow-lg shadow-[rgb(var(--bg-steel-rgb)/0.2)] transition-all flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Shield className="w-4 h-4" />
            {overriding ? 'Overriding...' : 'Strategic Override'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ConsensusOverrideModal;
