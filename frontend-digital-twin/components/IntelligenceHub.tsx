import React, { useEffect, useRef, useState, useCallback } from 'react';
import { Twin } from '../types';
import AgentVitality from './AgentVitality';
import NeuralMap from './NeuralMap';
import IntelligenceStream from './IntelligenceStream';
import EvolutionTimeline from './EvolutionTimeline';
import AttributionAnalytics from './AttributionAnalytics';
import IngestorStatus from './IngestorStatus';
import { useDomainAttribution } from '../context/DomainAttributionContext';

interface IntelligenceHubProps {
  orchestrator: Twin;
}

type NeuralMapView = 'atlas' | 'flow' | 'heatmap' | 'simulator' | 'playbooks' | 'evolution';

const IntelligenceHub: React.FC<IntelligenceHubProps> = ({ orchestrator }) => {
  const { incrementKnowledgeBase } = useDomainAttribution();
  const [neuralMapView, setNeuralMapView] = useState<NeuralMapView>('atlas');
  const [systemPaused, setSystemPaused] = useState<boolean>(false);
  const [selectedMemoryNodeId, setSelectedMemoryNodeId] = useState<string | null>(null);
  const [selectedAgentStationId, setSelectedAgentStationId] = useState<string | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);

  // Intelligence activity feed (SSE)
  const [recentTopics, setRecentTopics] = useState<string[]>([]);
  const [hasActiveMemoryTransfer, setHasActiveMemoryTransfer] = useState(false);
  const [hasActiveConsensusSession, setHasActiveConsensusSession] = useState(false);

  useEffect(() => {
    const sseUrl = '/api/phoenix/stream';
    const eventSource = new EventSource(sseUrl);
    eventSourceRef.current = eventSource;

    eventSource.addEventListener('memory_transfer', (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data);
        if (data.topic) {
          setRecentTopics((prev) => [String(data.topic), ...prev.filter((t) => t !== data.topic)].slice(0, 10));
          setHasActiveMemoryTransfer(true);
          window.setTimeout(() => setHasActiveMemoryTransfer(false), 2000);
        }
      } catch (error) {
        console.error('[IntelligenceHub] Failed to parse memory transfer event:', error);
      }
    });

    eventSource.addEventListener('consensus_vote', () => {
      setHasActiveConsensusSession(true);
    });

    eventSource.addEventListener('consensus_result', () => {
      // Leave indicator on until the operator visits/clears it.
    });

    eventSource.onerror = () => {
      // Keep quiet-ish: this endpoint may not be enabled in all envs.
      console.warn('[IntelligenceHub] SSE connection error');
    };

    return () => {
      eventSource.close();
      eventSourceRef.current = null;
    };
  }, []);

  // Handle Global System Pause (Kill-Switch)
  const handleSystemPause = useCallback(async () => {
    if (systemPaused) {
      // Resume system
      try {
        const response = await fetch('/api/phoenix/system/resume', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
        });
        if (response.ok) {
          setSystemPaused(false);
        } else {
          alert('Failed to resume system. Please check backend logs.');
        }
      } catch (err) {
        console.error('[IntelligenceHub] Failed to resume system:', err);
        alert('Failed to resume system. Please check backend logs.');
      }
    } else {
      // Pause system
      if (!confirm('⚠️ SYSTEM PAUSE: This will halt all autonomous deployments across all Agent Stations. Continue?')) {
        return;
      }
      try {
        const response = await fetch('/api/phoenix/system/pause', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
        });
        if (response.ok) {
          setSystemPaused(true);
        } else {
          alert('Failed to pause system. Please check backend logs.');
        }
      } catch (err) {
        console.error('[IntelligenceHub] Failed to pause system:', err);
        alert('Failed to pause system. Please check backend logs.');
      }
    }
  }, [systemPaused]);

  // Handle audit log click - center Neural Map on related memory node
  const handleAuditLogClick = useCallback((memoryNodeId: string) => {
    setSelectedMemoryNodeId(memoryNodeId);
    setNeuralMapView('atlas');
  }, []);

  // Handle Agent Station click - open SafeInstaller history
  const handleAgentStationClick = useCallback((agentStationId: string) => {
    setSelectedAgentStationId(agentStationId);
  }, []);

  return (
    <div className="flex-1 flex flex-col bg-[var(--bg-primary)] overflow-hidden relative">
      {/* Background Tactical Grid */}
      <div
        className="absolute inset-0 opacity-[0.03] pointer-events-none"
        style={{
          backgroundImage:
            'linear-gradient(rgb(var(--bg-steel-rgb)) 1px, transparent 1px), linear-gradient(90deg, rgb(var(--bg-steel-rgb)) 1px, transparent 1px)',
          backgroundSize: '40px 40px',
        }}
      />

      {/* Top rail - War Room Header */}
      <div className="relative z-10 flex items-center justify-between gap-3 px-4 py-3 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">psychology</span>
          <div className="min-w-0">
            <div className="text-[11px] font-black uppercase tracking-tight text-[var(--text-primary)]">
              Collective Intelligence Command Center
            </div>
            <div className="text-[9px] text-[var(--text-secondary)] font-bold uppercase tracking-widest truncate">
              Unified Agent Cluster Orchestration & Neural Telemetry
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Neural Map View Toggle */}
          <div className="flex items-center gap-1 border-r border-[rgb(var(--bg-steel-rgb)/0.3)] pr-2">
            <button
              onClick={() => setNeuralMapView('atlas')}
              className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] ${
                neuralMapView === 'atlas'
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                  : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]'
              }`}
            >
              <span className="material-symbols-outlined text-[14px] align-middle mr-1">account_tree</span>
              Atlas
            </button>
            <button
              onClick={() => setNeuralMapView('flow')}
              className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] ${
                neuralMapView === 'flow'
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                  : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]'
              }`}
            >
              <span className="material-symbols-outlined text-[14px] align-middle mr-1">timeline</span>
              Flow
            </button>
            <button
              onClick={() => setNeuralMapView('heatmap')}
              className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] ${
                neuralMapView === 'heatmap'
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                  : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]'
              }`}
            >
              <span className="material-symbols-outlined text-[14px] align-middle mr-1">grid_view</span>
              Heatmap
            </button>
            <button
              onClick={() => setNeuralMapView('simulator')}
              className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] ${
                neuralMapView === 'simulator'
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                  : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]'
              }`}
            >
              <span className="material-symbols-outlined text-[14px] align-middle mr-1">science</span>
              Simulator
            </button>
            <button
              onClick={() => setNeuralMapView('playbooks')}
              className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] ${
                neuralMapView === 'playbooks'
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                  : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]'
              }`}
            >
              <span className="material-symbols-outlined text-[14px] align-middle mr-1">menu_book</span>
              Playbooks
            </button>
            <button
              onClick={() => setNeuralMapView('evolution')}
              className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] ${
                neuralMapView === 'evolution'
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                  : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.75)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-secondary)]'
              }`}
            >
              <span className="material-symbols-outlined text-[14px] align-middle mr-1">timeline</span>
              Evolution
            </button>
          </div>

          {/* Global System Pause (Kill-Switch) */}
          <button
            onClick={handleSystemPause}
            className={`px-4 py-1.5 text-[10px] font-bold uppercase tracking-widest transition-all rounded-lg border-2 ${
              systemPaused
                ? 'bg-[rgb(var(--danger-rgb)/0.9)] border-[rgb(var(--danger-rgb))] text-white animate-pulse'
                : 'bg-[rgb(var(--surface-rgb)/0.4)] border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-primary)] hover:bg-[rgb(var(--warning-rgb)/0.2)] hover:border-[rgb(var(--warning-rgb)/0.5)]'
            }`}
            title={systemPaused ? 'System Paused - Click to Resume' : 'Pause All Autonomous Deployments'}
          >
            <span className="material-symbols-outlined text-[14px] align-middle mr-1">
              {systemPaused ? 'pause_circle' : 'play_circle'}
            </span>
            {systemPaused ? 'PAUSED' : 'System Pause'}
          </button>

          {/* Activity indicators */}
          <div className="flex items-center gap-1.5 pl-2 border-l border-[rgb(var(--bg-steel-rgb)/0.3)]">
            <div
              className={`w-2 h-2 rounded-full ${hasActiveMemoryTransfer ? 'bg-[var(--success)]' : 'bg-[rgb(var(--text-secondary-rgb)/0.35)]'}`}
              title={hasActiveMemoryTransfer ? 'Recent memory transfer detected' : 'No recent memory transfer'}
              style={
                hasActiveMemoryTransfer
                  ? { animation: 'pulse-glow 2s ease-in-out infinite', boxShadow: '0 0 8px rgba(var(--success-rgb), 0.8)' }
                  : undefined
              }
            />
            <div
              className={`w-2 h-2 rounded-full ${hasActiveConsensusSession ? 'bg-[var(--bg-steel)]' : 'bg-[rgb(var(--text-secondary-rgb)/0.35)]'}`}
              title={hasActiveConsensusSession ? 'Consensus session activity detected' : 'No consensus activity'}
              style={hasActiveConsensusSession ? { boxShadow: '0 0 6px rgba(var(--bg-steel-rgb), 0.6)' } : undefined}
            />
          </div>
        </div>
      </div>

      {/* Three-Column War Room Layout */}
      <div className="relative z-10 flex-1 min-h-0 p-4">
        <div className="grid grid-cols-12 gap-4 h-full min-h-0">
          {/* LEFT: Agent Vitality */}
          <div className="col-span-3 min-h-0 flex flex-col">
            <AgentVitality
              twinId={orchestrator.id}
              onAgentStationClick={handleAgentStationClick}
              selectedAgentStationId={selectedAgentStationId}
            />
          </div>

          {/* CENTER: Neural Map or Evolution Timeline */}
          <div className="col-span-6 min-h-0 flex flex-col">
            {neuralMapView === 'evolution' ? (
              <EvolutionTimeline />
            ) : (
              <NeuralMap
                view={neuralMapView}
                selectedMemoryNodeId={selectedMemoryNodeId}
                onMemoryNodeClick={(nodeId) => setSelectedMemoryNodeId(nodeId)}
              />
            )}
          </div>

          {/* RIGHT: Intelligence Stream & Attribution Analytics */}
          <div className="col-span-3 min-h-0 flex flex-col gap-4">
            <AttributionAnalytics className="shrink-0" />
            <IngestorStatus 
              className="shrink-0"
              onIngestionComplete={(domain, fileName) => {
                // Show notification in IntelligenceStream by adding to recentTopics
                const topic = `Ingestion: ${fileName} → ${domain}`;
                setRecentTopics(prev => [topic, ...prev.slice(0, 9)]);
                // Update knowledge base stats
                if (domain === 'Mind' || domain === 'Body' || domain === 'Heart' || domain === 'Soul') {
                  incrementKnowledgeBase(domain);
                }
              }}
            />
            <IntelligenceStream
              recentTopics={recentTopics}
              hasActiveConsensusSession={hasActiveConsensusSession}
              onAuditLogClick={handleAuditLogClick}
              onClear={() => {
                setRecentTopics([]);
                setHasActiveConsensusSession(false);
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
};

export default IntelligenceHub;
