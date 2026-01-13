import React, { useState, useEffect } from 'react';
import { Twin, TwinStatus, AppView } from '../types';
import { getAssetUrl, checkAssetExists } from '../services/assetService';
import { getUserName } from '../utils/userName';

interface SidebarLeftProps {
  twins: Twin[];
  activeTwinId: string;
  currentView: AppView;
  onSelectTwin: (id: string) => void;
  onSelectOrchestrator: () => void;
  onOpenCreateModal: () => void;
  onSelectSearch: () => void;
  onSelectMemoryExplorer: () => void;
  onSelectEvolution: () => void;
  onSelectSystemStatus: () => void;
  onSelectFileProcessingMonitor?: () => void;
  projects: { id: string; name: string; watchPath?: string }[];
  onSelectProject?: (projectId: string) => void;
  onCreateProject: (name: string) => void;
  onRenameProject: (projectId: string, name: string) => void;
  onDeleteProject: (projectId: string) => void;
  onConfigureWatchPath?: (projectId: string, watchPath: string) => void;
  onSelectOrchestratorSettings?: () => void;
}

const SidebarLeft: React.FC<SidebarLeftProps> = ({ twins, activeTwinId, currentView, onSelectTwin, onSelectOrchestrator, onOpenCreateModal, onSelectSearch, onSelectMemoryExplorer, onSelectEvolution, onSelectSystemStatus, onSelectFileProcessingMonitor, projects, onSelectProject, onCreateProject, onRenameProject, onDeleteProject, onConfigureWatchPath, onSelectOrchestratorSettings }) => {
  const [logoUrl, setLogoUrl] = useState('/ferrellgas-agi-badge.svg');
  const [userName, setUserName] = useState(getUserName());

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

  return (
    <aside className="w-64 bg-[#90C3EA] flex flex-col shrink-0 border-r border-[#5381A5]/30">
      <div className="p-4 border-b border-[#5381A5]/30 flex items-center gap-3">
        <img
          src={logoUrl}
          alt="Ferrellgas AGI"
          className="w-9 h-9 rounded shadow-lg shadow-indigo-500/20 shrink-0"
          loading="eager"
          decoding="async"
          onError={() => setLogoUrl('/ferrellgas-agi-badge.svg')}
        />
        <div className="flex-1 flex flex-col min-w-0">
          <span className="font-bold text-lg tracking-tight leading-none text-[#0b1b2b] font-display">Ferrellgas AGI</span>
          <span className="text-[9px] text-[#163247] font-bold tracking-wider uppercase mt-1">Tactical Agent Desktop</span>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto p-3 space-y-6">
        <div>
          <div className="text-[10px] font-bold text-[#163247] uppercase tracking-widest px-2 mb-2">Global Command</div>
          <div className="space-y-1">
            <button
              onClick={onSelectOrchestrator}
              className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                currentView === 'orchestrator' 
                  ? 'bg-[#5381A5] text-white border border-[#5381A5]' 
                  : 'text-[#0b1b2b] hover:bg-[#78A2C2] hover:text-[#0b1b2b] border border-transparent'
              }`}
            >
              <div className="p-1.5 bg-white/40 rounded-lg">
                <span className="material-symbols-outlined text-sm text-[#5381A5]">hub</span>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-bold uppercase tracking-wider">Ops Center</div>
                <div className="text-[9px] text-[#163247] truncate">The Blue Flame</div>
              </div>
            </button>
            <button
              onClick={onSelectSearch}
              className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                currentView === 'search' 
                  ? 'bg-[#5381A5] text-white border border-[#5381A5]' 
                  : 'text-[#0b1b2b] hover:bg-[#78A2C2] hover:text-[#0b1b2b] border border-transparent'
              }`}
            >
              <div className="p-1.5 bg-white/40 rounded-lg">
                <span className="material-symbols-outlined text-sm text-[#5381A5]">search</span>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-bold uppercase tracking-wider">Search Archive</div>
                <div className="text-[9px] text-[#163247] truncate">Global Intel scan</div>
              </div>
            </button>
          </div>
        </div>

        <div>
          <div className="text-[10px] font-bold text-[#163247] uppercase tracking-widest px-2 mb-2">System Admin</div>
          <div className="space-y-1">
            <button
              onClick={onSelectMemoryExplorer}
              className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                currentView === 'memory-explorer' 
                  ? 'bg-[#5381A5] text-white border border-[#5381A5]' 
                  : 'text-[#0b1b2b] hover:bg-[#78A2C2] hover:text-[#0b1b2b] border border-transparent'
              }`}
            >
              <div className="p-1.5 bg-white/40 rounded-lg">
                <span className="material-symbols-outlined text-sm text-[#5381A5]">database</span>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-bold uppercase tracking-wider">Memory Explorer</div>
                <div className="text-[9px] text-[#163247] truncate">Neural Archive</div>
              </div>
            </button>
            <button
              onClick={onSelectEvolution}
              className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                currentView === 'evolution' 
                  ? 'bg-[#5381A5] text-white border border-[#5381A5]' 
                  : 'text-[#0b1b2b] hover:bg-[#78A2C2] hover:text-[#0b1b2b] border border-transparent'
              }`}
            >
              <div className="p-1.5 bg-white/40 rounded-lg">
                <span className="material-symbols-outlined text-sm text-[#5381A5]">timeline</span>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-bold uppercase tracking-wider">Evolution</div>
                <div className="text-[9px] text-[#163247] truncate">Prompt Timeline</div>
              </div>
            </button>
            <button
              onClick={onSelectSystemStatus}
              className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                currentView === 'system-status' 
                  ? 'bg-[#5381A5] text-white border border-[#5381A5]' 
                  : 'text-[#0b1b2b] hover:bg-[#78A2C2] hover:text-[#0b1b2b] border border-transparent'
              }`}
            >
              <div className="p-1.5 bg-white/40 rounded-lg">
                <span className="material-symbols-outlined text-sm text-[#5381A5]">monitor</span>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-bold uppercase tracking-wider">System Status</div>
                <div className="text-[9px] text-[#163247] truncate">Sovereign Monitor</div>
              </div>
            </button>
          </div>
        </div>

        <div>
          <div className="text-[10px] font-bold text-[#163247] uppercase tracking-widest px-2 mb-2">Tactical Agents</div>
          <div className="space-y-1">
            {twins.filter(t => !t.isOrchestrator).map(twin => (
              <button
                key={twin.id}
                onClick={() => onSelectTwin(twin.id)}
                className={`w-full group flex items-start gap-3 p-2.5 rounded-xl transition-all text-left border border-transparent ${
                  activeTwinId === twin.id && currentView === 'chat' ? 'bg-[#78A2C2] text-[#0b1b2b] shadow-sm border-[#5381A5]/30' : 'text-[#0b1b2b] hover:bg-[#78A2C2]'
                }`}
              >
                <div className="relative shrink-0 mt-0.5">
                  <img src={twin.avatar} alt={twin.name} className="w-10 h-10 rounded-xl border border-[#5381A5]/30 object-cover" />
                  <span className={`absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-[#90C3EA] ${
                    twin.status === TwinStatus.THINKING ? 'bg-[#78A2C2] animate-pulse' :
                    twin.status === TwinStatus.IDLE ? 'bg-[#5381A5]' : 'bg-[#163247]'
                  }`} />
                </div>
                <div className="min-w-0 flex-1 flex flex-col gap-0.5">
                  <div className="text-[11px] font-black uppercase tracking-tight text-[#0b1b2b] truncate">{twin.name}</div>
                  <div className="text-[9px] text-[#5381A5] font-bold uppercase tracking-widest leading-none mb-1">{twin.role}</div>
                  
                  <div className="text-[10px] text-[#163247] line-clamp-2 leading-snug transition-colors">
                    {twin.description}
                  </div>
                </div>
              </button>
            ))}
            
            <button 
              onClick={onOpenCreateModal}
              className="w-full flex items-center gap-3 p-2.5 rounded-xl text-left text-[#163247] hover:bg-[#78A2C2] transition-all border border-dashed border-[#5381A5]/30 mt-2"
            >
              <div className="w-10 h-10 flex items-center justify-center rounded-xl bg-white/40 border border-[#5381A5]/30 shrink-0">
                <span className="material-symbols-outlined text-sm">add</span>
              </div>
              <div className="text-xs font-bold uppercase tracking-widest">New Agent</div>
            </button>

            {onSelectFileProcessingMonitor && (
              <button
                onClick={onSelectFileProcessingMonitor}
                className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                  currentView === 'file-processing-monitor' 
                    ? 'bg-[#5381A5] text-white border border-[#5381A5]' 
                    : 'bg-white/30 text-[#163247] hover:bg-[#78A2C2] border border-[#5381A5]/30'
                }`}
              >
                <div className="w-10 h-10 flex items-center justify-center rounded-xl bg-white/40 border border-[#5381A5]/30 shrink-0">
                  <span className="material-symbols-outlined text-sm">folder_managed</span>
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-xs font-bold uppercase tracking-wider">File Processing</div>
                  <div className="text-[9px] text-[#163247] truncate">Watch Monitor</div>
                </div>
              </button>
            )}
          </div>
        </div>

        <div>
          <div className="text-[10px] font-bold text-[#163247] uppercase tracking-widest px-2 mb-2">Monitored Applications</div>
          <div className="space-y-1">
            {projects.map((project) => (
              <div
                key={project.id}
                className="w-full flex items-center gap-2 p-2 rounded-lg text-left text-[#0b1b2b] hover:bg-[#78A2C2] transition-colors"
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
                <div className="w-4 h-4 rounded-sm bg-white/40 border border-[#5381A5]/30 shrink-0 flex items-center justify-center">
                  {project.watchPath && (
                    <span className="material-symbols-outlined text-[10px] text-[#5381A5]" title="Watch folder configured">
                      folder
                    </span>
                  )}
                </div>
                <span className="text-xs flex-1 min-w-0 truncate">{project.name}</span>

                <button
                  type="button"
                  className="p-1 rounded-md hover:bg-white/30 text-[#163247]"
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
                  className="p-1 rounded-md hover:bg-white/30 text-[#163247]"
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
                  className="p-1 rounded-md hover:bg-white/30 text-[#163247]"
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
              className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-[#163247] hover:bg-[#78A2C2] border border-dashed border-[#5381A5]/30 mt-2 transition-colors"
            >
              <div className="w-4 h-4 flex items-center justify-center rounded-sm bg-white/40 border border-[#5381A5]/30">
                <span className="material-symbols-outlined text-[10px]">add</span>
              </div>
              <span className="text-[11px] font-bold uppercase tracking-wider">Add Application</span>
            </button>
          </div>
        </div>
      </nav>

      <div className="p-4 border-t border-[#5381A5]/30">
        <div className="flex items-center gap-3 p-2 bg-white/30 rounded-xl border border-[#5381A5]/30">
           <div className="w-8 h-8 rounded-lg bg-gradient-to-tr from-[#78A2C2] to-[#5381A5] border border-[#5381A5]/30" />
           <div className="flex-1 min-w-0">
              <div className="text-[10px] font-black uppercase tracking-tight truncate">
                {userName}
              </div>
              <div className="text-[9px] text-[#163247] font-bold uppercase tracking-widest truncate">Authorized</div>
           </div>
           <button 
             onClick={() => onSelectOrchestratorSettings?.()}
             className="text-[#163247] hover:text-[#5381A5] transition-colors"
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
