import React, { useMemo, useState, useEffect } from 'react';
import { Brain } from 'lucide-react';
import { Twin, TwinStatus, AppView } from '../types';
import { getAssetUrl, checkAssetExists } from '../services/assetService';
import { listAgents } from '../services/agentService';
import { getUserName } from '../utils/userName';
import { getPendingToolProposals } from '../services/toolProposalService';

type CrewStatus = {
  agentCount: number | null;
  anyActive: boolean;
};

const CrewStatusBadge: React.FC<{ collapsed: boolean }> = ({ collapsed }) => {
  const [crewStatus, setCrewStatus] = useState<CrewStatus>({ agentCount: null, anyActive: false });

  useEffect(() => {
    let cancelled = false;
    let intervalId: number | undefined;
    let inFlight = false;

    const refresh = async () => {
      if (inFlight) return;
      inFlight = true;

      try {
        const res = await listAgents();
        if (cancelled) return;
        const agents = res?.agents ?? [];
        const anyActive = agents.some((a) => (a.status || '').toLowerCase() !== 'idle');
        setCrewStatus({ agentCount: agents.length, anyActive });
      } catch {
        // Silent failure: don't break navigation/UI if backend isn't reachable.
        if (cancelled) return;
        setCrewStatus({ agentCount: null, anyActive: false });
      } finally {
        inFlight = false;
      }
    };

    refresh();
    intervalId = window.setInterval(refresh, 10_000);

    return () => {
      cancelled = true;
      if (intervalId !== undefined) window.clearInterval(intervalId);
    };
  }, []);

  const pillText = useMemo(() => {
    if (crewStatus.agentCount === null) return '?';
    return String(crewStatus.agentCount);
  }, [crewStatus.agentCount]);

  // When collapsed, we only show the activity dot (if any) to keep the sidebar compact.
  if (collapsed) {
    if (!crewStatus.anyActive) return null;
    return (
      <span
        className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-emerald-400 animate-pulse ring-2 ring-[var(--bg-secondary)]"
        aria-label="Crew active"
      />
    );
  }

  return (
    <span className="flex items-center gap-1.5 shrink-0">
      {crewStatus.anyActive && (
        <span
          className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"
          aria-label="Crew active"
        />
      )}
      <span className="text-xs bg-blue-500/20 text-blue-400 px-1.5 py-0.5 rounded-full border border-blue-500/30 leading-none">
        {pillText}
      </span>
    </span>
  );
};

interface SidebarLeftProps {
  twins: Twin[];
  activeTwinId: string;
  currentView: AppView;
  onSelectTwin: (id: string) => void;
  onSelectOrchestrator: () => void;
  onSelectIntelligenceHub?: () => void;
  onOpenCreateModal: () => void;
  onSelectSearch: () => void;
  onSelectMemoryExplorer: () => void;
  onSelectEvolution: () => void;
  onSelectSystemStatus: () => void;
  onSelectPhoenix?: () => void;
  onSelectFileProcessingMonitor?: () => void;
  onSelectAgentForge?: () => void;
  onSelectToolForge?: () => void;
  onSelectAudit?: () => void;
  onSelectKnowledgeAtlas?: () => void;
  projects: { id: string; name: string; watchPath?: string }[];
  onSelectProject?: (projectId: string) => void;
  onCreateProject: (name: string) => void;
  onRenameProject: (projectId: string, name: string) => void;
  onDeleteProject: (projectId: string) => void;
  onConfigureWatchPath?: (projectId: string, watchPath: string) => void;
  onSelectOrchestratorSettings?: () => void;
  onOpenToolProposals?: () => void;
}

const SidebarLeft: React.FC<SidebarLeftProps> = ({ twins, activeTwinId, currentView, onSelectTwin, onSelectOrchestrator, onSelectIntelligenceHub, onOpenCreateModal, onSelectSearch, onSelectMemoryExplorer, onSelectEvolution, onSelectSystemStatus, onSelectPhoenix, onSelectFileProcessingMonitor, onSelectAgentForge, onSelectToolForge, onSelectAudit, onSelectKnowledgeAtlas, projects, onSelectProject, onCreateProject, onRenameProject, onDeleteProject, onConfigureWatchPath, onSelectOrchestratorSettings, onOpenToolProposals }) => {
  const [logoUrl, setLogoUrl] = useState('/ferrellgas-agi-badge.svg');
  const [userName, setUserName] = useState(getUserName());
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [pendingProposalsCount, setPendingProposalsCount] = useState(0);

  const orchestratorName = twins.find(t => t.isOrchestrator)?.name || 'Phoenix';

  useEffect(() => {
    // Try to load custom logo (supports svg/png/jpg), fallback to default.
    const loadCustomLogo = async () => {
      const candidates = [
        'custom-logo.png',
        'custom-logo.jpg',
        'custom-logo.jpeg',
        'custom-logo.svg',
      ];

      for (const filename of candidates) {
        const url = getAssetUrl(filename);
        // eslint-disable-next-line no-await-in-loop
        const exists = await checkAssetExists(url);
        if (exists) {
          setLogoUrl(`${url}?v=${Date.now()}`);
          return;
        }
      }

      setLogoUrl('/ferrellgas-agi-badge.svg');
    };

    const handleLogoChanged = (e: CustomEvent) => {
      const url = (e.detail?.url as string | undefined) || '';
      if (url) {
        setLogoUrl(url);
      } else {
        loadCustomLogo();
      }
    };

    loadCustomLogo();
    window.addEventListener('logoChanged', handleLogoChanged as EventListener);
    window.addEventListener('focus', loadCustomLogo);
    return () => {
      window.removeEventListener('logoChanged', handleLogoChanged as EventListener);
      window.removeEventListener('focus', loadCustomLogo);
    };
  }, []);

  // Listen for user name changes
  useEffect(() => {
    const handleStorageChange = () => {
      setUserName(getUserName());
    };
    const handleUserNameChanged = (e: CustomEvent) => {
      setUserName(getUserName());
    };
    window.addEventListener('storage', handleStorageChange);
    window.addEventListener('userNameChanged', handleUserNameChanged as EventListener);
    // Also check on focus in case it changed in another tab
    window.addEventListener('focus', handleStorageChange);
    return () => {
      window.removeEventListener('storage', handleStorageChange);
      window.removeEventListener('userNameChanged', handleUserNameChanged as EventListener);
      window.removeEventListener('focus', handleStorageChange);
    };
  }, []);

  // Fetch pending tool proposals count
  useEffect(() => {
    const fetchPendingProposals = async () => {
      try {
        const response = await getPendingToolProposals();
        setPendingProposalsCount(response.proposals.length);
      } catch (err) {
        console.error('[SidebarLeft] Failed to fetch pending proposals:', err);
      }
    };

    fetchPendingProposals();
    // Refresh every 30 seconds
    const interval = setInterval(fetchPendingProposals, 30000);
    return () => clearInterval(interval);
  }, []);

  return (
    <aside className={`${isCollapsed ? 'w-16' : 'w-64'} bg-[var(--bg-secondary)] flex flex-col shrink-0 border-r border-[rgb(var(--bg-steel-rgb)/0.3)] transition-all duration-300`}>
      <div className={`p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center gap-3 ${isCollapsed ? 'justify-center' : ''}`}>
        <img
          src={logoUrl}
          alt="Ferrellgas AGI"
          className="w-9 h-9 rounded shadow-lg [--tw-shadow-color:rgb(var(--accent-rgb)/0.2)] shrink-0"
          loading="eager"
          decoding="async"
          onError={() => setLogoUrl('/ferrellgas-agi-badge.svg')}
        />
        {!isCollapsed && (
          <div className="flex-1 flex flex-col min-w-0">
            <span className="font-bold text-lg tracking-tight leading-none text-[var(--text-primary)] font-display">Ferrellgas AGI</span>
            <span className="text-[9px] text-[var(--text-secondary)] font-bold tracking-wider uppercase mt-1">Tactical Agent Desktop</span>
          </div>
        )}
        <div className="flex items-center gap-2">
          {onOpenToolProposals && pendingProposalsCount > 0 && (
            <button
              onClick={onOpenToolProposals}
              className="relative p-1.5 hover:bg-[var(--bg-muted)] rounded-lg transition-colors text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
              title={`${pendingProposalsCount} pending tool installation proposal(s)`}
            >
              <span className={`material-symbols-outlined text-sm ${pendingProposalsCount > 0 ? 'text-yellow-500' : ''}`}>
                notifications
              </span>
              {pendingProposalsCount > 0 && (
                <span className="absolute -top-0.5 -right-0.5 w-4 h-4 bg-yellow-500 rounded-full flex items-center justify-center text-[8px] font-bold text-white animate-pulse">
                  {pendingProposalsCount > 9 ? '9+' : pendingProposalsCount}
                </span>
              )}
            </button>
          )}
          <button
            onClick={() => setIsCollapsed(!isCollapsed)}
            className="p-1.5 hover:bg-[var(--bg-muted)] rounded-lg transition-colors text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
            title={isCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            <span className="material-symbols-outlined text-sm">
              {isCollapsed ? 'chevron_right' : 'chevron_left'}
            </span>
          </button>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto p-3 space-y-6">
        <div>
          {!isCollapsed && (
            <div className="text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-widest px-2 mb-2">Global Command</div>
          )}
          <div className="space-y-1">
            <button
              onClick={onSelectOrchestrator}
              className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                currentView === 'orchestrator' 
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                  : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
              }`}
              title={isCollapsed ? `Ops Center - ${orchestratorName}` : undefined}
            >
              <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">hub</span>
              </div>
              {!isCollapsed && (
                <div className="min-w-0">
                  <div className="text-xs font-bold uppercase tracking-wider">Ops Center</div>
                  <div className="text-[9px] text-[var(--text-secondary)] truncate">{orchestratorName}</div>
                </div>
              )}
            </button>

            {onSelectIntelligenceHub && (
              <button
                onClick={onSelectIntelligenceHub}
                className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                  currentView === 'intelligence-hub'
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]'
                    : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
                }`}
                title={isCollapsed ? 'Intelligence Hub - Agent Orchestration' : undefined}
              >
                <div className="relative p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                  <Brain size={16} className="text-[var(--bg-steel)]" />
                  <CrewStatusBadge collapsed={isCollapsed} />
                </div>
                {!isCollapsed && (
                  <div className="min-w-0">
                    <div className="text-xs font-bold uppercase tracking-wider flex items-center gap-2">
                      <span className="min-w-0 truncate">Intelligence Hub</span>
                      <CrewStatusBadge collapsed={isCollapsed} />
                    </div>
                    <div className="text-[9px] text-[var(--text-secondary)] truncate">Agent orchestration</div>
                  </div>
                )}
              </button>
            )}

            <button
              onClick={onSelectSearch}
              className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                currentView === 'search' 
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                  : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
              }`}
              title={isCollapsed ? 'Search Archive - Global Intel scan' : undefined}
            >
              <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">search</span>
              </div>
              {!isCollapsed && (
                <div className="min-w-0">
                  <div className="text-xs font-bold uppercase tracking-wider">Search Archive</div>
                  <div className="text-[9px] text-[var(--text-secondary)] truncate">Global Intel scan</div>
                </div>
              )}
            </button>
          </div>
        </div>

        <div>
          {!isCollapsed && (
            <div className="text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-widest px-2 mb-2">System Admin</div>
          )}
          <div className="space-y-1">
            <button
              onClick={onSelectMemoryExplorer}
              className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                currentView === 'memory-explorer' 
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                  : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
              }`}
              title={isCollapsed ? 'Memory Explorer - Neural Archive' : undefined}
            >
              <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">database</span>
              </div>
              {!isCollapsed && (
                <div className="min-w-0">
                  <div className="text-xs font-bold uppercase tracking-wider">Memory Explorer</div>
                  <div className="text-[9px] text-[var(--text-secondary)] truncate">Neural Archive</div>
                </div>
              )}
            </button>
            {onSelectKnowledgeAtlas && (
              <button
                onClick={onSelectKnowledgeAtlas}
                className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                  currentView === 'knowledge-atlas' 
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                    : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
                }`}
                title={isCollapsed ? 'Knowledge Atlas - 3D Memory Map' : undefined}
              >
                <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                  <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">account_tree</span>
                </div>
                {!isCollapsed && (
                  <div className="min-w-0">
                    <div className="text-xs font-bold uppercase tracking-wider">Knowledge Atlas</div>
                    <div className="text-[9px] text-[var(--text-secondary)] truncate">3D Memory Map</div>
                  </div>
                )}
              </button>
            )}
            <button
              onClick={onSelectEvolution}
              className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                currentView === 'evolution' 
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                  : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
              }`}
              title={isCollapsed ? 'Evolution - Prompt Timeline' : undefined}
            >
              <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">timeline</span>
              </div>
              {!isCollapsed && (
                <div className="min-w-0">
                  <div className="text-xs font-bold uppercase tracking-wider">Evolution</div>
                  <div className="text-[9px] text-[var(--text-secondary)] truncate">Prompt Timeline</div>
                </div>
              )}
            </button>
            {onSelectPhoenix && (
              <button
                onClick={onSelectPhoenix}
                className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                  currentView === 'phoenix' 
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                    : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
                }`}
                title={isCollapsed ? 'Phoenix Monitor - Collective Intelligence' : undefined}
              >
                <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                  <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">psychology</span>
                </div>
                {!isCollapsed && (
                  <div className="min-w-0">
                    <div className="text-xs font-bold uppercase tracking-wider">Phoenix Monitor</div>
                    <div className="text-[9px] text-[var(--text-secondary)] truncate">Collective Intelligence</div>
                  </div>
                )}
              </button>
            )}
            <button
              onClick={onSelectSystemStatus}
              className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                currentView === 'system-status' 
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                  : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
              }`}
              title={isCollapsed ? 'System Status - Health Dashboard' : undefined}
            >
              <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">monitor</span>
              </div>
              {!isCollapsed && (
                <div className="min-w-0">
                  <div className="text-xs font-bold uppercase tracking-wider">System Status</div>
                  <div className="text-[9px] text-[var(--text-secondary)] truncate">Health Dashboard</div>
                </div>
              )}
            </button>
            {onSelectAudit && (
              <button
                onClick={onSelectAudit}
                className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                  currentView === 'audit' 
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                    : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
                }`}
                title={isCollapsed ? 'Audit Dashboard - Governance & Hygiene' : undefined}
              >
                <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                  <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">assessment</span>
                </div>
                {!isCollapsed && (
                  <div className="min-w-0">
                    <div className="text-xs font-bold uppercase tracking-wider">Audit Dashboard</div>
                    <div className="text-[9px] text-[var(--text-secondary)] truncate">Governance & Hygiene</div>
                  </div>
                )}
              </button>
            )}
            {onSelectAgentForge && (
              <button
                onClick={onSelectAgentForge}
                className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                  currentView === 'agent-forge'
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]'
                    : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
                }`}
                title={isCollapsed ? 'AgentForge - Build Agents' : undefined}
              >
                <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                  <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">engineering</span>
                </div>
                {!isCollapsed && (
                  <div className="min-w-0">
                    <div className="text-xs font-bold uppercase tracking-wider">AgentForge</div>
                    <div className="text-[9px] text-[var(--text-secondary)] truncate">Build Agents</div>
                  </div>
                )}
              </button>
            )}
            {onSelectToolForge && (
              <button
                onClick={onSelectToolForge}
                className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                  currentView === 'tool-forge'
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]'
                    : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-primary)] border border-transparent'
                }`}
                title={isCollapsed ? 'ToolForge - Build Tools' : undefined}
              >
                <div className="p-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg">
                  <span className="material-symbols-outlined text-sm text-[var(--bg-steel)]">construction</span>
                </div>
                {!isCollapsed && (
                  <div className="min-w-0">
                    <div className="text-xs font-bold uppercase tracking-wider">ToolForge</div>
                    <div className="text-[9px] text-[var(--text-secondary)] truncate">Build Tools</div>
                  </div>
                )}
              </button>
            )}
          </div>
        </div>

        <div>
          {!isCollapsed && (
            <div className="text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-widest px-2 mb-2">Tactical Agents</div>
          )}
          <div className="space-y-1">
            {twins.filter(t => !t.isOrchestrator).map(twin => (
              <button
                key={twin.id}
                onClick={() => onSelectTwin(twin.id)}
                className={`w-full group flex items-${isCollapsed ? 'center justify-center' : 'start gap-3'} p-2.5 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} border border-transparent ${
                  activeTwinId === twin.id && currentView === 'chat' ? 'bg-[var(--bg-muted)] text-[var(--text-primary)] shadow-sm border-[rgb(var(--bg-steel-rgb)/0.3)]' : 'text-[var(--text-primary)] hover:bg-[var(--bg-muted)]'
                }`}
                title={isCollapsed ? `${twin.name} - ${twin.role}` : undefined}
              >
                <div className="relative shrink-0 mt-0.5">
                  <img src={twin.avatar} alt={twin.name} className="w-10 h-10 rounded-xl border border-[rgb(var(--bg-steel-rgb)/0.3)] object-cover" />
                  <span className={`absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-[var(--bg-secondary)] ${
                    twin.status === TwinStatus.THINKING ? 'bg-[var(--bg-muted)] animate-pulse' :
                    twin.status === TwinStatus.IDLE ? 'bg-[var(--bg-steel)]' : 'bg-[var(--text-secondary)]'
                  }`} />
                </div>
                {!isCollapsed && (
                  <div className="min-w-0 flex-1 flex flex-col gap-0.5">
                    <div className="text-[11px] font-black uppercase tracking-tight text-[var(--text-primary)] truncate">{twin.name}</div>
                    <div className="text-[9px] text-[var(--bg-steel)] font-bold uppercase tracking-widest leading-none mb-1">{twin.role}</div>
                    
                    <div className="text-[10px] text-[var(--text-secondary)] line-clamp-2 leading-snug transition-colors">
                      {twin.description}
                    </div>
                  </div>
                )}
              </button>
            ))}
            
            <button 
              onClick={onOpenCreateModal}
              className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-2.5 rounded-xl ${isCollapsed ? '' : 'text-left'} text-[var(--text-secondary)] hover:bg-[var(--bg-muted)] transition-all border border-dashed border-[rgb(var(--bg-steel-rgb)/0.3)] mt-2`}
              title={isCollapsed ? 'New Agent' : undefined}
            >
              <div className="w-10 h-10 flex items-center justify-center rounded-xl bg-[rgb(var(--surface-rgb)/0.4)] border border-[rgb(var(--bg-steel-rgb)/0.3)] shrink-0">
                <span className="material-symbols-outlined text-sm">add</span>
              </div>
              {!isCollapsed && (
                <div className="text-xs font-bold uppercase tracking-widest">New Agent</div>
              )}
            </button>

            {onSelectFileProcessingMonitor && (
              <button
                onClick={onSelectFileProcessingMonitor}
                className={`w-full flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-3 rounded-xl transition-all ${isCollapsed ? '' : 'text-left'} ${
                  currentView === 'file-processing-monitor' 
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border border-[var(--bg-steel)]' 
                    : 'bg-[rgb(var(--surface-rgb)/0.3)] text-[var(--text-secondary)] hover:bg-[var(--bg-muted)] border border-[rgb(var(--bg-steel-rgb)/0.3)]'
                }`}
                title={isCollapsed ? 'File Processing - Watch Monitor' : undefined}
              >
                <div className="w-10 h-10 flex items-center justify-center rounded-xl bg-[rgb(var(--surface-rgb)/0.4)] border border-[rgb(var(--bg-steel-rgb)/0.3)] shrink-0">
                  <span className="material-symbols-outlined text-sm">folder_managed</span>
                </div>
                {!isCollapsed && (
                  <div className="flex-1 min-w-0">
                    <div className="text-xs font-bold uppercase tracking-wider">File Processing</div>
                    <div className="text-[9px] text-[var(--text-secondary)] truncate">Watch Monitor</div>
                  </div>
                )}
              </button>
            )}
          </div>
        </div>

        {!isCollapsed && (
          <div>
            <div className="text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-widest px-2 mb-2">Monitored Applications</div>
            <div className="space-y-1">
              {projects.map((project) => (
                <div
                  key={project.id}
                  className="w-full flex items-center gap-2 p-2 rounded-lg text-left text-[var(--text-primary)] hover:bg-[var(--bg-muted)] transition-colors"
                  role="button"
                  tabIndex={0}
                  onClick={() => onSelectProject?.(project.id)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      onSelectProject?.(project.id);
                    }
                  }}
                  title={project.id}
                >
                  <div className="w-4 h-4 rounded-sm bg-[rgb(var(--surface-rgb)/0.4)] border border-[rgb(var(--bg-steel-rgb)/0.3)] shrink-0 flex items-center justify-center">
                    {project.watchPath && (
                      <span className="material-symbols-outlined text-[10px] text-[var(--bg-steel)]" title="Watch folder configured">
                        folder
                      </span>
                    )}
                  </div>
                  <span className="text-xs flex-1 min-w-0 truncate">{project.name}</span>

                  <button
                    type="button"
                    className="p-1 rounded-md hover:bg-[rgb(var(--surface-rgb)/0.3)] text-[var(--text-secondary)]"
                    title={project.watchPath ? `Watch folder: ${project.watchPath}` : "Configure Watch Folder"}
                    onClick={(e) => {
                      e.stopPropagation();
                      const currentPath = project.watchPath || '';
                      const newPath = window.prompt(
                        `Configure watch folder for ${project.name}:\n\nEnter the local file system path to monitor for logs, emails, and alerts.\n\nLeave empty to remove watch folder.`,
                        currentPath
                      );
                      if (newPath !== null && onConfigureWatchPath) {
                        onConfigureWatchPath(project.id, newPath.trim());
                      }
                    }}
                  >
                    <span className="material-symbols-outlined text-[14px]">{project.watchPath ? 'folder_open' : 'folder'}</span>
                  </button>

                  <button
                    type="button"
                    className="p-1 rounded-md hover:bg-[rgb(var(--surface-rgb)/0.3)] text-[var(--text-secondary)]"
                    title="Rename Application"
                    onClick={(e) => {
                      e.stopPropagation();
                      const nextName = window.prompt('Rename application', project.name);
                      if (!nextName) return;
                      const trimmed = nextName.trim();
                      if (!trimmed) return;
                      onRenameProject(project.id, trimmed);
                    }}
                  >
                    <span className="material-symbols-outlined text-[14px]">edit</span>
                  </button>

                  <button
                    type="button"
                    className="p-1 rounded-md hover:bg-[rgb(var(--surface-rgb)/0.3)] text-[var(--text-secondary)]"
                    title="Delete Application"
                    onClick={(e) => {
                      e.stopPropagation();
                      const ok = window.confirm(`Delete application "${project.name}"?`);
                      if (!ok) return;
                      onDeleteProject(project.id);
                    }}
                  >
                    <span className="material-symbols-outlined text-[14px]">delete</span>
                  </button>
                </div>
              ))}
              
              <button 
                onClick={() => {
                  const name = window.prompt('New application name (e.g., Rapid7 SIEM, Zscaler, CrowdStrike, Splunk, etc.)');
                  if (!name) return;
                  const trimmed = name.trim();
                  if (!trimmed) return;
                  onCreateProject(trimmed);
                }}
                className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-[var(--text-secondary)] hover:bg-[var(--bg-muted)] border border-dashed border-[rgb(var(--bg-steel-rgb)/0.3)] mt-2 transition-colors"
              >
                <div className="w-4 h-4 flex items-center justify-center rounded-sm bg-[rgb(var(--surface-rgb)/0.4)] border border-[rgb(var(--bg-steel-rgb)/0.3)]">
                  <span className="material-symbols-outlined text-[10px]">add</span>
                </div>
                <span className="text-[11px] font-bold uppercase tracking-wider">Add Application</span>
              </button>
            </div>
          </div>
        )}
      </nav>

      <div className="p-4 border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
        <div className={`flex items-center ${isCollapsed ? 'justify-center' : 'gap-3'} p-2 bg-[rgb(var(--surface-rgb)/0.3)] rounded-xl border border-[rgb(var(--bg-steel-rgb)/0.3)]`}>
           <div className="w-8 h-8 rounded-lg bg-gradient-to-tr from-[var(--bg-muted)] to-[var(--bg-steel)] border border-[rgb(var(--bg-steel-rgb)/0.3)]" />
           {!isCollapsed && (
             <div className="flex-1 min-w-0">
                <div className="text-[10px] font-black uppercase tracking-tight truncate">
                  {userName}
                </div>
                <div className="text-[9px] text-[var(--text-secondary)] font-bold uppercase tracking-widest truncate">Authorized</div>
             </div>
           )}
           <button 
             onClick={() => onSelectOrchestratorSettings?.()}
             className="text-[var(--text-secondary)] hover:text-[var(--bg-steel)] transition-colors"
             title="Orchestrator Settings"
           >
              <span className="material-symbols-outlined text-sm">settings</span>
           </button>
        </div>
      </div>
    </aside>
  );
};

export default SidebarLeft;
