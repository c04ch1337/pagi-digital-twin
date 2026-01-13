import React, { useState, useEffect } from 'react';
import { Twin, TwinStatus, AppView } from '../types';
import { getCustomLogoUrl, checkAssetExists } from '../services/assetService';

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
}

const SidebarLeft: React.FC<SidebarLeftProps> = ({ twins, activeTwinId, currentView, onSelectTwin, onSelectOrchestrator, onOpenCreateModal, onSelectSearch, onSelectMemoryExplorer, onSelectEvolution, onSelectSystemStatus }) => {
  const [logoUrl, setLogoUrl] = useState('/ferrellgas-agi-badge.svg');

  useEffect(() => {
    // Try to load custom logo, fallback to default
    const loadCustomLogo = async () => {
      const customLogoUrl = getCustomLogoUrl();
      const exists = await checkAssetExists(customLogoUrl);
      if (exists) {
        setLogoUrl(customLogoUrl);
      } else {
        setLogoUrl('/ferrellgas-agi-badge.svg');
      }
    };
    loadCustomLogo();
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
                <div className="text-[9px] text-[#163247] truncate">Orchestrator Hub</div>
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
                    twin.status === TwinStatus.THINKING ? 'bg-amber-500 animate-pulse' :
                    twin.status === TwinStatus.IDLE ? 'bg-emerald-500' : 'bg-zinc-600'
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
          </div>
        </div>

        <div>
          <div className="text-[10px] font-bold text-[#163247] uppercase tracking-widest px-2 mb-2">Active Projects</div>
          <div className="space-y-1">
            <button className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-[#0b1b2b] hover:bg-[#78A2C2]">
              <div className="w-4 h-4 rounded-sm bg-white/40 border border-[#5381A5]/30" />
              <span className="text-xs">Project Alpha</span>
            </button>
            <button className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-[#0b1b2b] hover:bg-[#78A2C2]">
              <div className="w-4 h-4 rounded-sm bg-white/40 border border-[#5381A5]/30" />
              <span className="text-xs">Neural Sync</span>
            </button>
            
            <button className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-[#163247] hover:bg-[#78A2C2] border border-dashed border-[#5381A5]/30 mt-2">
              <div className="w-4 h-4 flex items-center justify-center rounded-sm bg-white/40 border border-[#5381A5]/30">
                <span className="material-symbols-outlined text-[10px]">add</span>
              </div>
              <span className="text-[11px] font-bold uppercase tracking-wider">Create New Project</span>
            </button>
          </div>
        </div>
      </nav>

      <div className="p-4 border-t border-[#5381A5]/30">
        <div className="flex items-center gap-3 p-2 bg-white/30 rounded-xl border border-[#5381A5]/30">
           <div className="w-8 h-8 rounded-lg bg-gradient-to-tr from-[#78A2C2] to-[#5381A5] border border-[#5381A5]/30" />
           <div className="flex-1 min-w-0">
              <div className="text-[10px] font-black uppercase tracking-tight truncate">Root Admin</div>
              <div className="text-[9px] text-[#163247] font-bold uppercase tracking-widest truncate">Authorized</div>
           </div>
           <button className="text-[#163247] hover:text-[#5381A5] transition-colors">
              <span className="material-symbols-outlined text-sm">settings</span>
           </button>
        </div>
      </div>
    </aside>
  );
};

export default SidebarLeft;
