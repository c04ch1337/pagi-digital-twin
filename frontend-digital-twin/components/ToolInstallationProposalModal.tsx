import React, { useState, useEffect } from 'react';
import { ToolInstallationProposal, getToolProposals, approveToolProposal, rejectToolProposal, simulateToolProposal, SimulationResult } from '../services/toolProposalService';
import { getPeerReviewsForProposal, PeerReview } from '../services/peerReviewService';

interface ToolInstallationProposalModalProps {
  isOpen: boolean;
  onClose: () => void;
  onProposalUpdated?: () => void;
}

const ToolInstallationProposalModal: React.FC<ToolInstallationProposalModalProps> = ({
  isOpen,
  onClose,
  onProposalUpdated,
}) => {
  const [proposals, setProposals] = useState<ToolInstallationProposal[]>([]);
  const [loading, setLoading] = useState(false);
  const [processingId, setProcessingId] = useState<string | null>(null);
  const [selectedProposal, setSelectedProposal] = useState<ToolInstallationProposal | null>(null);
  const [simulationResult, setSimulationResult] = useState<SimulationResult | null>(null);
  const [simulatingId, setSimulatingId] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      fetchProposals();
    }
  }, [isOpen]);

  const fetchProposals = async () => {
    setLoading(true);
    try {
      const response = await getToolProposals();
      // Filter to show pending proposals first, then others
      const sorted = response.proposals.sort((a, b) => {
        if (a.status === 'pending' && b.status !== 'pending') return -1;
        if (a.status !== 'pending' && b.status === 'pending') return 1;
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      });
      setProposals(sorted);
      if (sorted.length > 0 && !selectedProposal) {
        setSelectedProposal(sorted[0]);
      }
    } catch (err) {
      console.error('[ToolInstallationProposalModal] Failed to fetch proposals:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleApprove = async (proposalId: string) => {
    if (!confirm('Are you sure you want to approve this tool installation? This will execute the installation command on your system.')) {
      return;
    }

    setProcessingId(proposalId);
    try {
      await approveToolProposal(proposalId);
      await fetchProposals();
      if (onProposalUpdated) {
        onProposalUpdated();
      }
      // Select next pending proposal if available
      const nextPending = proposals.find(p => p.id !== proposalId && p.status === 'pending');
      setSelectedProposal(nextPending || null);
    } catch (err) {
      console.error('[ToolInstallationProposalModal] Failed to approve proposal:', err);
      alert('Failed to approve proposal. Please try again.');
    } finally {
      setProcessingId(null);
    }
  };

  const handleReject = async (proposalId: string) => {
    if (!confirm('Are you sure you want to reject this tool installation proposal?')) {
      return;
    }

    setProcessingId(proposalId);
    try {
      await rejectToolProposal(proposalId);
      await fetchProposals();
      if (onProposalUpdated) {
        onProposalUpdated();
      }
      // Select next pending proposal if available
      const nextPending = proposals.find(p => p.id !== proposalId && p.status === 'pending');
      setSelectedProposal(nextPending || null);
    } catch (err) {
      console.error('[ToolInstallationProposalModal] Failed to reject proposal:', err);
      alert('Failed to reject proposal. Please try again.');
    } finally {
      setProcessingId(null);
    }
  };

  const handleSimulate = async (proposalId: string) => {
    setSimulatingId(proposalId);
    setSimulationResult(null);
    try {
      const result = await simulateToolProposal(proposalId);
      if (result.ok && result.simulation) {
        setSimulationResult(result.simulation);
        if (result.simulation.success) {
          // Show success message
          alert('Simulation successful! The tool can be safely installed.');
        } else {
          // Show warning
          alert(`Simulation failed: ${result.simulation.message}\n\nReview the errors before approving.`);
        }
      } else {
        alert(`Simulation failed: ${result.error || result.message || 'Unknown error'}`);
      }
    } catch (err) {
      console.error('[ToolInstallationProposalModal] Failed to simulate proposal:', err);
      alert('Failed to run simulation. Please try again.');
    } finally {
      setSimulatingId(null);
    }
  };

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  if (!isOpen) return null;

  const pendingProposals = proposals.filter(p => p.status === 'pending');
  const otherProposals = proposals.filter(p => p.status !== 'pending');

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-[var(--bg-primary)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] w-full max-w-4xl max-h-[90vh] flex flex-col overflow-hidden">
        {/* Header */}
        <div className="p-6 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-[var(--bg-steel)]">construction</span>
            <h2 className="text-xl font-bold text-[var(--text-primary)] uppercase tracking-tight">
              Tool Installation Proposals
            </h2>
            {pendingProposals.length > 0 && (
              <span className="px-2 py-1 bg-yellow-500/20 text-yellow-500 rounded-full text-xs font-bold">
                {pendingProposals.length} Pending
              </span>
            )}
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-[var(--bg-muted)] rounded-lg transition-colors text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
          >
            <span className="material-symbols-outlined">close</span>
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-hidden flex">
          {/* Proposal List */}
          <div className="w-80 border-r border-[rgb(var(--bg-steel-rgb)/0.3)] overflow-y-auto bg-[var(--bg-secondary)]">
            {loading ? (
              <div className="p-4 text-center text-[var(--bg-steel)]">
                <span className="material-symbols-outlined animate-spin">hourglass_empty</span>
                <p className="mt-2 text-sm">Loading proposals...</p>
              </div>
            ) : proposals.length === 0 ? (
              <div className="p-4 text-center text-[var(--bg-steel)]">
                <span className="material-symbols-outlined text-4xl mb-2">inbox</span>
                <p className="text-sm">No proposals available</p>
              </div>
            ) : (
              <div className="p-2 space-y-2">
                {pendingProposals.length > 0 && (
                  <>
                    <div className="px-2 py-1 text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-widest">
                      Pending ({pendingProposals.length})
                    </div>
                    {pendingProposals.map((proposal) => (
                      <button
                        key={proposal.id}
                        onClick={() => {
                          setSelectedProposal(proposal);
                          setSimulationResult(null); // Clear simulation when switching proposals
                        }}
                        className={`w-full text-left p-3 rounded-lg transition-colors border ${
                          selectedProposal?.id === proposal.id
                            ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border-[var(--bg-steel)]'
                            : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--text-primary)] border-[rgb(var(--bg-steel-rgb)/0.2)] hover:bg-[var(--bg-muted)]'
                        }`}
                      >
                        <div className="font-semibold text-sm mb-1 truncate">{proposal.tool_name}</div>
                        <div className="text-xs opacity-80 truncate">{proposal.repository}</div>
                        <div className="text-xs mt-1 flex items-center gap-2">
                          <span className="material-symbols-outlined text-xs">star</span>
                          {proposal.stars.toLocaleString()}
                          {proposal.language && (
                            <>
                              <span className="mx-1">•</span>
                              <span>{proposal.language}</span>
                            </>
                          )}
                        </div>
                      </button>
                    ))}
                  </>
                )}
                {otherProposals.length > 0 && (
                  <>
                    <div className="px-2 py-1 text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-widest mt-4">
                      Reviewed ({otherProposals.length})
                    </div>
                    {otherProposals.map((proposal) => (
                      <button
                        key={proposal.id}
                        onClick={() => {
                          setSelectedProposal(proposal);
                          setSimulationResult(null); // Clear simulation when switching proposals
                        }}
                        className={`w-full text-left p-3 rounded-lg transition-colors border ${
                          selectedProposal?.id === proposal.id
                            ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border-[var(--bg-steel)]'
                            : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--text-primary)] border-[rgb(var(--bg-steel-rgb)/0.2)] hover:bg-[var(--bg-muted)] opacity-60'
                        }`}
                      >
                        <div className="font-semibold text-sm mb-1 truncate">{proposal.tool_name}</div>
                        <div className="text-xs opacity-80 truncate">{proposal.repository}</div>
                        <div className="text-xs mt-1 flex items-center gap-2">
                          <span className={`px-1.5 py-0.5 rounded text-[10px] font-bold ${
                            proposal.status === 'approved' 
                              ? 'bg-green-500/20 text-green-500' 
                              : 'bg-red-500/20 text-red-500'
                          }`}>
                            {proposal.status.toUpperCase()}
                          </span>
                        </div>
                      </button>
                    ))}
                  </>
                )}
              </div>
            )}
          </div>

          {/* Proposal Details */}
          <div className="flex-1 overflow-y-auto p-6">
            {selectedProposal ? (
              <div className="space-y-4">
                <div>
                  <h3 className="text-lg font-bold text-[var(--text-primary)] mb-2">{selectedProposal.tool_name}</h3>
                  <div className="flex items-center gap-4 text-sm text-[var(--bg-steel)] mb-4">
                    <a
                      href={selectedProposal.github_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1 text-[var(--bg-steel)] hover:text-[var(--text-primary)] transition-colors"
                    >
                      <span className="material-symbols-outlined text-sm">open_in_new</span>
                      {selectedProposal.repository}
                    </a>
                    <div className="flex items-center gap-1">
                      <span className="material-symbols-outlined text-sm">star</span>
                      {selectedProposal.stars.toLocaleString()} stars
                    </div>
                    {selectedProposal.language && (
                      <span className="px-2 py-0.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded text-xs">
                        {selectedProposal.language}
                      </span>
                    )}
                  </div>
                </div>

                <div>
                  <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2">Description</h4>
                  <p className="text-sm text-[var(--text-primary)] bg-[rgb(var(--bg-secondary-rgb)/1)] p-3 rounded">
                    {selectedProposal.description}
                  </p>
                </div>

                {/* Consensus Confidence Meter */}
                <ConsensusConfidenceMeter proposalId={selectedProposal.id} />

                <div>
                  <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2">Installation Command</h4>
                  <div className="bg-[rgb(var(--bg-secondary-rgb)/1)] p-3 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)]">
                    <code className="text-sm font-mono text-[var(--text-primary)] break-all">
                      {selectedProposal.installation_command}
                    </code>
                  </div>
                </div>

                <div>
                  <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2">Code Snippet</h4>
                  <pre className="bg-[rgb(var(--bg-secondary-rgb)/1)] p-3 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)] overflow-x-auto">
                    <code className="text-xs font-mono text-[var(--text-primary)]">
                      {selectedProposal.code_snippet}
                    </code>
                  </pre>
                </div>

                {/* Verification Status */}
                {(selectedProposal.installation_success !== undefined || selectedProposal.verified !== undefined) && (
                  <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] pt-4">
                    <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2 flex items-center gap-2">
                      <span className="material-symbols-outlined text-base">verified</span>
                      Installation Status
                    </h4>
                    <div className="space-y-2">
                      {selectedProposal.installation_success !== undefined && (
                        <div className="flex items-center gap-2">
                          <span className={`px-2 py-1 rounded text-xs font-bold ${
                            selectedProposal.installation_success
                              ? 'bg-green-500/20 text-green-500'
                              : 'bg-red-500/20 text-red-500'
                          }`}>
                            {selectedProposal.installation_success ? 'INSTALLED' : 'INSTALLATION FAILED'}
                          </span>
                        </div>
                      )}
                      {selectedProposal.verified !== undefined && (
                        <div className="flex items-center gap-2">
                          <span className={`px-2 py-1 rounded text-xs font-bold ${
                            selectedProposal.verified
                              ? 'bg-green-500/20 text-green-500'
                              : 'bg-yellow-500/20 text-yellow-500'
                          }`}>
                            {selectedProposal.verified ? 'VERIFIED' : 'VERIFICATION FAILED'}
                          </span>
                        </div>
                      )}
                      {selectedProposal.verification_message && (
                        <p className="text-xs text-[var(--text-secondary)] bg-[rgb(var(--bg-secondary-rgb)/1)] p-2 rounded">
                          {selectedProposal.verification_message}
                        </p>
                      )}
                    </div>
                  </div>
                )}

                {/* Simulation Result Section */}
                {simulationResult && selectedProposal && (
                  <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] pt-4">
                    <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2 flex items-center gap-2">
                      <span className={`material-symbols-outlined text-base ${
                        simulationResult.success ? 'text-green-500' : 'text-red-500'
                      }`}>
                        {simulationResult.success ? 'check_circle' : 'error'}
                      </span>
                      Simulation Result
                    </h4>
                    <div className={`border rounded p-4 space-y-3 ${
                      simulationResult.success
                        ? 'bg-green-500/10 border-green-500/30'
                        : 'bg-red-500/10 border-red-500/30'
                    }`}>
                      <div>
                        <p className={`text-sm font-semibold ${
                          simulationResult.success ? 'text-green-500' : 'text-red-500'
                        }`}>
                          {simulationResult.message}
                        </p>
                      </div>

                      {simulationResult.errors.length > 0 && (
                        <div>
                          <p className="text-xs text-[var(--text-secondary)] mb-1">Errors:</p>
                          <ul className="list-disc list-inside text-xs text-[var(--text-primary)] space-y-1">
                            {simulationResult.errors.map((error, idx) => (
                              <li key={idx}>{error}</li>
                            ))}
                          </ul>
                        </div>
                      )}

                      <details className="text-xs">
                        <summary className="cursor-pointer text-[var(--text-secondary)] hover:text-[var(--text-primary)] mb-2">
                          Installation Output
                        </summary>
                        <pre className="bg-[rgb(var(--bg-primary-rgb)/1)] p-3 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)] overflow-x-auto max-h-40 overflow-y-auto">
                          <code className="text-xs font-mono text-[var(--text-primary)] whitespace-pre-wrap">
                            {simulationResult.installation_output || 'No output'}
                          </code>
                        </pre>
                      </details>

                      <details className="text-xs">
                        <summary className="cursor-pointer text-[var(--text-secondary)] hover:text-[var(--text-primary)] mb-2">
                          Verification Output
                        </summary>
                        <pre className="bg-[rgb(var(--bg-primary-rgb)/1)] p-3 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)] overflow-x-auto max-h-40 overflow-y-auto">
                          <code className="text-xs font-mono text-[var(--text-primary)] whitespace-pre-wrap">
                            {simulationResult.verification_output || 'No output'}
                          </code>
                        </pre>
                      </details>
                    </div>
                  </div>
                )}

                {/* Repair Proposal Section */}
                {selectedProposal.repair_proposal && (
                  <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] pt-4">
                    <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2 flex items-center gap-2">
                      <span className="material-symbols-outlined text-base text-yellow-500">healing</span>
                      Suggested Fix
                    </h4>
                    <div className="bg-yellow-500/10 border border-yellow-500/30 rounded p-4 space-y-3">
                      <div>
                        <p className="text-xs text-[var(--text-secondary)] mb-1">Repair Reason:</p>
                        <p className="text-sm text-[var(--text-primary)]">
                          {selectedProposal.repair_proposal.repair_reason}
                        </p>
                      </div>
                      
                      <div>
                        <p className="text-xs text-[var(--text-secondary)] mb-1">Rollback Command:</p>
                        <div className="bg-[rgb(var(--bg-primary-rgb)/1)] p-3 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)]">
                          <code className="text-sm font-mono text-[var(--text-primary)] break-all">
                            {selectedProposal.repair_proposal.rollback_command}
                          </code>
                        </div>
                      </div>

                      {selectedProposal.repair_proposal.last_successful_timestamp && (
                        <div className="flex items-center gap-2 text-xs text-[var(--text-secondary)]">
                          <span className="material-symbols-outlined text-sm">history</span>
                          <span>Last successful state: {formatTimestamp(selectedProposal.repair_proposal.last_successful_timestamp)}</span>
                        </div>
                      )}

                      {selectedProposal.repair_proposal.last_successful_command && (
                        <div>
                          <p className="text-xs text-[var(--text-secondary)] mb-1">Last Successful Command:</p>
                          <div className="bg-[rgb(var(--bg-primary-rgb)/1)] p-2 rounded border border-[rgb(var(--bg-steel-rgb)/0.2)]">
                            <code className="text-xs font-mono text-[var(--text-primary)] break-all">
                              {selectedProposal.repair_proposal.last_successful_command}
                            </code>
                          </div>
                        </div>
                      )}

                      <div className="flex items-center gap-2 text-xs">
                        <span className="text-[var(--text-secondary)]">Confidence:</span>
                        <div className="flex-1 bg-[rgb(var(--bg-steel-rgb)/0.2)] rounded-full h-2 overflow-hidden">
                          <div
                            className="h-full bg-yellow-500 transition-all"
                            style={{ width: `${(selectedProposal.repair_proposal.confidence * 100)}%` }}
                          />
                        </div>
                        <span className="text-[var(--text-secondary)]">
                          {Math.round(selectedProposal.repair_proposal.confidence * 100)}%
                        </span>
                      </div>
                    </div>
                  </div>
                )}

                <div className="text-xs text-[var(--bg-steel)] space-y-1">
                  <p><strong>Proposed by:</strong> {selectedProposal.agent_name}</p>
                  <p><strong>Created:</strong> {formatTimestamp(selectedProposal.created_at)}</p>
                  {selectedProposal.reviewed_at && (
                    <p><strong>Reviewed:</strong> {formatTimestamp(selectedProposal.reviewed_at)}</p>
                  )}
                </div>

                {selectedProposal.status === 'pending' && (
                  <div className="space-y-3 pt-4 border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
                    <div className="flex gap-3">
                      <button
                        onClick={() => handleSimulate(selectedProposal.id)}
                        disabled={simulatingId === selectedProposal.id || processingId === selectedProposal.id}
                        className="flex-1 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                      >
                        <span className="material-symbols-outlined text-sm">
                          {simulatingId === selectedProposal.id ? 'hourglass_empty' : 'science'}
                        </span>
                        {simulatingId === selectedProposal.id ? 'Simulating...' : 'Simulate in Sandbox'}
                      </button>
                      <button
                        onClick={() => handleReject(selectedProposal.id)}
                        disabled={processingId === selectedProposal.id || simulatingId === selectedProposal.id}
                        className="flex-1 px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                      >
                        <span className="material-symbols-outlined text-sm">
                          {processingId === selectedProposal.id ? 'hourglass_empty' : 'cancel'}
                        </span>
                        {processingId === selectedProposal.id ? 'Rejecting...' : 'Reject'}
                      </button>
                    </div>
                    <button
                      onClick={() => handleApprove(selectedProposal.id)}
                      disabled={processingId === selectedProposal.id || simulatingId === selectedProposal.id}
                      className={`w-full px-4 py-2 text-white rounded hover:bg-green-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 ${
                        simulationResult?.success
                          ? 'bg-green-600'
                          : 'bg-green-600/70'
                      }`}
                    >
                      <span className="material-symbols-outlined text-sm">
                        {processingId === selectedProposal.id ? 'hourglass_empty' : 'check_circle'}
                      </span>
                      {processingId === selectedProposal.id 
                        ? 'Approving...' 
                        : simulationResult?.success
                          ? 'Approve & Deploy to Fleet'
                          : 'Approve & Deploy (Simulation Recommended)'}
                    </button>
                  </div>
                )}
              </div>
            ) : (
              <div className="text-center py-12 text-[var(--bg-steel)]">
                <span className="material-symbols-outlined text-4xl mb-2">info</span>
                <p>Select a proposal to view details</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

/// Consensus Confidence Meter Component
interface ConsensusConfidenceMeterProps {
  proposalId: string;
}

const ConsensusConfidenceMeter: React.FC<ConsensusConfidenceMeterProps> = ({ proposalId }) => {
  const [reviews, setReviews] = useState<PeerReview[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchReviews = async () => {
      try {
        setLoading(true);
        const response = await getPeerReviewsForProposal(proposalId);
        setReviews(response.reviews || []);
      } catch (err) {
        console.error('[ConsensusConfidenceMeter] Failed to fetch reviews:', err);
        setReviews([]);
      } finally {
        setLoading(false);
      }
    };

    if (proposalId) {
      fetchReviews();
      // Refresh every 5 seconds
      const interval = setInterval(fetchReviews, 5000);
      return () => clearInterval(interval);
    }
  }, [proposalId]);

  if (loading) {
    return (
      <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] pt-4">
        <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2 flex items-center gap-2">
          <span className="material-symbols-outlined text-base">forum</span>
          Agent Debate Consensus
        </h4>
        <div className="text-xs text-[var(--text-secondary)]">Loading consensus data...</div>
      </div>
    );
  }

  if (reviews.length === 0) {
    return (
      <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] pt-4">
        <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2 flex items-center gap-2">
          <span className="material-symbols-outlined text-base">forum</span>
          Agent Debate Consensus
        </h4>
        <div className="text-xs text-[var(--text-secondary)] opacity-70">
          No peer reviews yet. Peer review will be automatically requested when this proposal is created.
        </div>
      </div>
    );
  }

  // Calculate consensus confidence
  const completedReviews = reviews.filter(r => r.expert_decision);
  const concurCount = completedReviews.filter(r => r.expert_decision === 'concur').length;
  const objectCount = completedReviews.filter(r => r.expert_decision === 'object').length;
  const totalCompleted = completedReviews.length;
  const confidence = totalCompleted > 0 ? (concurCount / totalCompleted) : 0;
  const consensus = completedReviews.find(r => r.consensus)?.consensus;

  return (
    <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] pt-4">
      <h4 className="text-sm font-semibold text-[var(--text-primary)] mb-2 flex items-center gap-2">
        <span className="material-symbols-outlined text-base">forum</span>
        Agent Debate Consensus
      </h4>
      
      <div className="space-y-3">
        {/* Confidence Meter */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-[var(--text-secondary)]">Consensus Confidence</span>
            <span className={`text-xs font-bold ${
              confidence >= 0.7 ? 'text-[var(--success)]' :
              confidence >= 0.4 ? 'text-[rgb(var(--warning-rgb))]' :
              'text-[rgb(var(--danger-rgb))]'
            }`}>
              {Math.round(confidence * 100)}%
            </span>
          </div>
          <div className="w-full bg-[rgb(var(--bg-steel-rgb)/0.2)] rounded-full h-3 overflow-hidden">
            <div
              className={`h-full transition-all ${
                confidence >= 0.7 ? 'bg-[var(--success)]' :
                confidence >= 0.4 ? 'bg-[rgb(var(--warning-rgb))]' :
                'bg-[rgb(var(--danger-rgb))]'
              }`}
              style={{ width: `${confidence * 100}%` }}
            />
          </div>
        </div>

        {/* Consensus Status */}
        {consensus && (
          <div className={`px-3 py-2 rounded border ${
            consensus === 'approved'
              ? 'bg-[var(--success)]/20 border-[var(--success)]/40 text-[var(--success)]'
              : 'bg-[rgb(var(--warning-rgb)/0.2)] border-[rgb(var(--warning-rgb)/0.4)] text-[rgb(var(--warning-rgb))]'
          }`}>
            <div className="flex items-center gap-2 text-xs font-bold">
              <span className="material-symbols-outlined text-sm">
                {consensus === 'approved' ? 'check_circle' : 'cancel'}
              </span>
              Consensus: {consensus.toUpperCase()}
            </div>
          </div>
        )}

        {/* Review Summary */}
        <div className="text-xs text-[var(--text-secondary)] space-y-1">
          <div className="flex items-center gap-4">
            <span>Total Reviews: {reviews.length}</span>
            <span>Completed: {totalCompleted}</span>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-[var(--success)]">Concur: {concurCount}</span>
            <span className="text-[rgb(var(--warning-rgb))]">Object: {objectCount}</span>
          </div>
        </div>

        {/* Review Details */}
        {reviews.map((review) => (
          <div
            key={review.review_id}
            className="bg-[rgb(var(--bg-secondary-rgb)/0.5)] border border-[rgb(var(--bg-steel-rgb)/0.2)] rounded p-2 space-y-1"
          >
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-[var(--text-primary)]">
                {review.expert_agent_name}
              </span>
              {review.expert_decision && (
                <span className={`px-2 py-0.5 rounded text-[10px] font-bold ${
                  review.expert_decision === 'concur'
                    ? 'bg-[var(--success)]/20 text-[var(--success)]'
                    : 'bg-[rgb(var(--warning-rgb)/0.2)] text-[rgb(var(--warning-rgb))]'
                }`}>
                  {review.expert_decision.toUpperCase()}
                </span>
              )}
            </div>
            {review.expert_reasoning && (
              <p className="text-xs text-[var(--text-secondary)] line-clamp-2">
                {review.expert_reasoning}
              </p>
            )}
            {review.alternative_playbook_id && (
              <p className="text-xs text-[var(--bg-steel)] italic">
                Alternative playbook suggested: {review.alternative_playbook_id}
              </p>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};

export default ToolInstallationProposalModal;
