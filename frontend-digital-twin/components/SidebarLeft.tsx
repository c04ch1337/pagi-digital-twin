import React from 'react';
import { Twin, TwinStatus, AppView } from '../types';

interface SidebarLeftProps {
  twins: Twin[];
  activeTwinId: string;
  currentView: AppView;
  onSelectTwin: (id: string) => void;
  onSelectOrchestrator: () => void;
  onOpenCreateModal: () => void;
  onSelectSearch: () => void;
}

const SidebarLeft: React.FC<SidebarLeftProps> = ({ twins, activeTwinId, currentView, onSelectTwin, onSelectOrchestrator, onOpenCreateModal, onSelectSearch }) => {
  return (
    <aside className="w-64 bg-zinc-950 flex flex-col shrink-0 border-r border-zinc-800/50">
      <div className="p-4 border-b border-zinc-800/50 flex items-center gap-3">
        <img
          src="/ferrellgas-agi-badge.svg"
          alt="Ferrellgas AGI"
          className="w-9 h-9 rounded shadow-lg shadow-indigo-500/20 shrink-0"
          loading="eager"
          decoding="async"
        />
        <div className="flex-1 flex flex-col min-w-0">
          <span className="font-bold text-lg tracking-tight leading-none text-white font-display">Ferrellgas AGI</span>
          <span className="text-[9px] text-zinc-500 font-bold tracking-wider uppercase mt-1">Tactical Node Network</span>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto p-3 space-y-6">
        <div>
          <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest px-2 mb-2">Global Command</div>
          <div className="space-y-1">
            <button
              onClick={onSelectOrchestrator}
              className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                currentView === 'orchestrator' 
                  ? 'bg-indigo-600/20 text-indigo-400 border border-indigo-600/30' 
                  : 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200 border border-transparent'
              }`}
            >
              <div className="p-1.5 bg-indigo-600/10 rounded-lg">
                <span className="material-symbols-outlined text-sm text-indigo-400">hub</span>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-bold uppercase tracking-wider">Ops Center</div>
                <div className="text-[9px] text-zinc-500 truncate">Orchestrator Hub</div>
              </div>
            </button>
            <button
              onClick={onSelectSearch}
              className={`w-full flex items-center gap-3 p-3 rounded-xl transition-all text-left ${
                currentView === 'search' 
                  ? 'bg-indigo-600/20 text-indigo-400 border border-indigo-600/30' 
                  : 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200 border border-transparent'
              }`}
            >
              <div className="p-1.5 bg-indigo-600/10 rounded-lg">
                <span className="material-symbols-outlined text-sm text-indigo-400">search</span>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-bold uppercase tracking-wider">Search Archive</div>
                <div className="text-[9px] text-zinc-500 truncate">Global Intel scan</div>
              </div>
            </button>
          </div>
        </div>

        <div>
          <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest px-2 mb-2">Tactical Agents</div>
          <div className="space-y-1">
            {twins.filter(t => !t.isOrchestrator).map(twin => (
              <button
                key={twin.id}
                onClick={() => onSelectTwin(twin.id)}
                className={`w-full group flex items-start gap-3 p-2.5 rounded-xl transition-all text-left border border-transparent ${
                  activeTwinId === twin.id && currentView === 'chat' ? 'bg-zinc-900 text-white shadow-sm border-zinc-800' : 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200'
                }`}
              >
                <div className="relative shrink-0 mt-0.5">
                  <img src={twin.avatar} alt={twin.name} className="w-10 h-10 rounded-xl border border-zinc-800 object-cover" />
                  <span className={`absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-zinc-950 ${
                    twin.status === TwinStatus.THINKING ? 'bg-amber-500 animate-pulse' :
                    twin.status === TwinStatus.IDLE ? 'bg-emerald-500' : 'bg-zinc-600'
                  }`} />
                </div>
                <div className="min-w-0 flex-1 flex flex-col gap-0.5">
                  <div className="text-[11px] font-black uppercase tracking-tight text-zinc-100 truncate">{twin.name}</div>
                  <div className="text-[9px] text-indigo-400 font-bold uppercase tracking-widest leading-none mb-1">{twin.role}</div>
                  
                  <div className="text-[10px] text-zinc-500 line-clamp-2 leading-snug group-hover:text-zinc-400 transition-colors">
                    {twin.description}
                  </div>
                </div>
              </button>
            ))}
            
            <button 
              onClick={onOpenCreateModal}
              className="w-full flex items-center gap-3 p-2.5 rounded-xl text-left text-zinc-500 hover:bg-zinc-900 hover:text-indigo-400 transition-all border border-dashed border-zinc-800 mt-2"
            >
              <div className="w-10 h-10 flex items-center justify-center rounded-xl bg-zinc-900 border border-zinc-800 shrink-0">
                <span className="material-symbols-outlined text-sm">add</span>
              </div>
              <div className="text-xs font-bold uppercase tracking-widest">New Tactical Node</div>
            </button>
          </div>
        </div>

        <div>
          <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest px-2 mb-2">Active Projects</div>
          <div className="space-y-1">
            <button className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200">
              <div className="w-4 h-4 rounded-sm bg-zinc-800 border border-zinc-700" />
              <span className="text-xs">Project Alpha</span>
            </button>
            <button className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200">
              <div className="w-4 h-4 rounded-sm bg-zinc-800 border border-zinc-700" />
              <span className="text-xs">Neural Sync</span>
            </button>
            
            <button className="w-full flex items-center gap-3 p-2 rounded-lg text-left text-zinc-600 hover:bg-zinc-900 hover:text-zinc-400 border border-dashed border-zinc-800 mt-2">
              <div className="w-4 h-4 flex items-center justify-center rounded-sm bg-zinc-900 border border-zinc-800">
                <span className="material-symbols-outlined text-[10px]">add</span>
              </div>
              <span className="text-[11px] font-bold uppercase tracking-wider">Create New Project</span>
            </button>
          </div>
        </div>
      </nav>

      <div className="p-4 border-t border-zinc-800/50">
        <div className="flex items-center gap-3 p-2 bg-zinc-950/50 rounded-xl border border-zinc-800">
           <div className="w-8 h-8 rounded-lg bg-gradient-to-tr from-zinc-700 to-zinc-500 border border-zinc-600" />
           <div className="flex-1 min-w-0">
              <div className="text-[10px] font-black uppercase tracking-tight truncate">Root Admin</div>
              <div className="text-[9px] text-zinc-600 font-bold uppercase tracking-widest truncate">Authorized</div>
           </div>
           <button className="text-zinc-500 hover:text-indigo-400 transition-colors">
              <span className="material-symbols-outlined text-sm">settings</span>
           </button>
        </div>
      </div>
    </aside>
  );
};

export default SidebarLeft;
