import React, { useState, useEffect } from 'react';
import { getPendingToolProposals, ToolInstallationProposal, simulateToolProposal, SimulationResult } from '../services/toolProposalService';

interface ToolSimulatorProps {
  className?: string;
}

const ToolSimulator: React.FC<ToolSimulatorProps> = ({ className = '' }) => {
  const [proposals, setProposals] = useState<ToolInstallationProposal[]>([]);
  const [selectedProposal, setSelectedProposal] = useState<ToolInstallationProposal | null>(null);
  const [simulationResult, setSimulationResult] = useState<SimulationResult | null>(null);
  const [simulating, setSimulating] = useState<boolean>(false);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchProposals = async () => {
      try {
        setLoading(true);
        setError(null);
        const response = await getPendingToolProposals();
        setProposals(response.proposals);
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to fetch tool proposals';
        setError(msg);
        console.error('[ToolSimulator] Failed to fetch proposals:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchProposals();
    // Refresh every 10 seconds
    const interval = setInterval(fetchProposals, 10000);
    return () => clearInterval(interval);
  }, []);

  const handleSimulate = async (proposal: ToolInstallationProposal) => {
    setSimulating(true);
    setSimulationResult(null);
    setSelectedProposal(proposal);

    try {
      const result = await simulateToolProposal(proposal.id);
      if (result.ok && result.simulation) {
        setSimulationResult(result.simulation);
      } else {
        setError(result.error || result.message || 'Simulation failed');
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to run simulation';
      setError(msg);
      console.error('[ToolSimulator] Simulation error:', err);
    } finally {
      setSimulating(false);
    }
  };

  const formatTimestamp = (timestamp: string) => {
    try {
      return new Date(timestamp).toLocaleString();
    } catch {
      return timestamp;
    }
  };

  if (loading && proposals.length === 0) {
    return (
      <div className={`${className} flex items-center justify-center h-full`}>
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">
          Loading tool proposals...
        </div>
      </div>
    );
  }

  return (
    <div className={`${className} h-full flex flex-col overflow-hidden`}>
      <div className="mb-3">
        <h4 className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
          Simulation Sandbox
        </h4>
        <p className="text-[9px] text-[var(--text-secondary)] opacity-70">
          Test tool installation proposals in an isolated environment before deployment
        </p>
      </div>

      {error && (
        <div className="mb-3 text-[11px] text-[rgb(var(--danger-rgb)/0.85)] bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--danger-rgb)/0.3)] rounded-lg px-3 py-2">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-y-auto space-y-3">
        {proposals.length === 0 ? (
          <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">
            No pending tool proposals available for simulation
          </div>
        ) : (
          proposals.map((proposal) => (
            <div
              key={proposal.id}
              className={`bg-[rgb(var(--surface-rgb)/0.7)] border-2 rounded-lg p-3 transition-all ${
                selectedProposal?.id === proposal.id
                  ? 'border-[var(--bg-steel)] bg-[rgb(var(--bg-steel-rgb)/0.1)]'
                  : 'border-[rgb(var(--bg-steel-rgb)/0.3)] hover:border-[var(--bg-muted)]'
              }`}
            >
              <div className="flex items-start justify-between gap-2 mb-2">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <span className="text-[11px] font-bold text-[var(--text-primary)] truncate">
                      {proposal.tool_name}
                    </span>
                    {proposal.language && (
                      <span className="text-[9px] px-1.5 py-0.5 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded font-mono">
                        {proposal.language}
                      </span>
                    )}
                  </div>
                  <div className="text-[10px] text-[var(--text-secondary)] opacity-80 mb-1 line-clamp-2">
                    {proposal.description}
                  </div>
                  <div className="text-[9px] text-[var(--bg-steel)] font-mono mb-1">
                    {proposal.installation_command}
                  </div>
                  <div className="text-[9px] text-[var(--text-secondary)] opacity-60">
                    Proposed by: {proposal.agent_name} • {formatTimestamp(proposal.created_at)}
                  </div>
                </div>
                <button
                  onClick={() => handleSimulate(proposal)}
                  disabled={simulating}
                  className="px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] text-[var(--text-on-accent)] rounded transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {simulating && selectedProposal?.id === proposal.id ? (
                    <>
                      <span className="material-symbols-outlined text-[14px] align-middle mr-1 animate-spin">hourglass_empty</span>
                      Simulating...
                    </>
                  ) : (
                    <>
                      <span className="material-symbols-outlined text-[14px] align-middle mr-1">science</span>
                      Simulate
                    </>
                  )}
                </button>
              </div>

              {/* Simulation Result */}
              {selectedProposal?.id === proposal.id && simulationResult && (
                <div className="mt-3 pt-3 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                  <div className={`mb-2 px-2 py-1 rounded text-[10px] font-bold ${
                    simulationResult.success
                      ? 'bg-[var(--success)] text-white'
                      : 'bg-[rgb(var(--danger-rgb))] text-white'
                  }`}>
                    {simulationResult.success ? '✓ Simulation Successful' : '✗ Simulation Failed'}
                  </div>
                  <div className="text-[9px] text-[var(--text-secondary)] mb-2">
                    {simulationResult.message}
                  </div>
                  
                  {simulationResult.installation_output && (
                    <details className="mb-2">
                      <summary className="text-[9px] font-semibold text-[var(--text-secondary)] cursor-pointer mb-1">
                        Installation Output
                      </summary>
                      <pre className="text-[8px] bg-[rgb(var(--bg-secondary-rgb)/0.5)] p-2 rounded overflow-x-auto max-h-32 overflow-y-auto font-mono">
                        {simulationResult.installation_output}
                      </pre>
                    </details>
                  )}

                  {simulationResult.verification_output && (
                    <details className="mb-2">
                      <summary className="text-[9px] font-semibold text-[var(--text-secondary)] cursor-pointer mb-1">
                        Verification Output
                      </summary>
                      <pre className="text-[8px] bg-[rgb(var(--bg-secondary-rgb)/0.5)] p-2 rounded overflow-x-auto max-h-32 overflow-y-auto font-mono">
                        {simulationResult.verification_output}
                      </pre>
                    </details>
                  )}

                  {simulationResult.errors.length > 0 && (
                    <div className="mt-2">
                      <div className="text-[9px] font-semibold text-[rgb(var(--danger-rgb))] mb-1">
                        Errors:
                      </div>
                      <ul className="list-disc list-inside text-[8px] text-[rgb(var(--danger-rgb))] space-y-0.5">
                        {simulationResult.errors.map((err, idx) => (
                          <li key={idx}>{err}</li>
                        ))}
                      </ul>
                    </div>
                  )}

                  <div className="text-[8px] text-[var(--text-secondary)] opacity-60 mt-2">
                    Sandbox: {simulationResult.sandbox_path}
                  </div>
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
};

export default ToolSimulator;
