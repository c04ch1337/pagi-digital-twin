import React, { useState, useEffect, useCallback } from 'react';
import { Twin, Message, Job, Approval, TwinStatus, TelemetryData, AppView, LogEntry } from './types';
import { INITIAL_TWINS, ICONS, AVAILABLE_TOOLS } from './constants';
import SidebarLeft from './components/SidebarLeft';
import SidebarRight from './components/SidebarRight';
import ChatArea from './components/ChatArea';
import SettingsView from './components/SettingsView';
import OrchestratorHub from './components/OrchestratorHub';
import JobLogsView from './components/JobLogsView';
import CreateTwinModal from './components/CreateTwinModal';
import SearchView from './components/SearchView';
import { generateAgentResponse } from './services/gemini';
import { executeJobLifecycle } from './services/orchestrator';

const App: React.FC = () => {
  const [twins, setTwins] = useState<Twin[]>(INITIAL_TWINS);
  const [activeTwinId, setActiveTwinId] = useState<string>(INITIAL_TWINS[0].id);
  const [view, setView] = useState<AppView>('orchestrator');
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [approvals, setApprovals] = useState<Approval[]>([]);
  const [telemetry, setTelemetry] = useState<TelemetryData[]>([]);
  const [isSidebarLeftOpen, setIsSidebarLeftOpen] = useState(true);
  const [isSidebarRightOpen, setIsSidebarRightOpen] = useState(true);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);

  const activeTwin = twins.find(t => t.id === activeTwinId) || twins[0];
  const orchestrator = twins.find(t => t.isOrchestrator) || twins[0];
  const selectedJob = jobs.find(j => j.id === selectedJobId);

  useEffect(() => {
    const interval = setInterval(() => {
      const newData: TelemetryData = {
        cpu: Math.floor(Math.random() * 40) + 10,
        memory: Math.floor(Math.random() * 30) + 40,
        gpu: Math.floor(Math.random() * 20) + 5,
        network: Math.floor(Math.random() * 100),
        timestamp: new Date().toLocaleTimeString(),
      };
      setTelemetry(prev => [...prev.slice(-19), newData]);
    }, 2000);
    return () => clearInterval(interval);
  }, []);

  // Centralized job updater to avoid race conditions in state updates
  const updateJobInState = useCallback((updatedJob: Job) => {
    setJobs(prev => prev.map(j => j.id === updatedJob.id ? updatedJob : j));
  }, []);

  const handleSendMessage = async (text: string, twinOverride?: Twin) => {
    const targetTwin = twinOverride || activeTwin;
    const userMsg: Message = {
      id: Date.now().toString(),
      sender: 'user',
      content: text,
      timestamp: new Date(),
      twinId: targetTwin.id,
    };
    setMessages(prev => [...prev, userMsg]);

    setTwins(prev => prev.map(t => t.id === targetTwin.id ? { ...t, status: TwinStatus.THINKING } : t));

    try {
      const response = await generateAgentResponse(
        targetTwin,
        text,
        messages.slice(-5).filter(m => m.twinId === targetTwin.id || !m.twinId).map(m => ({ role: m.sender, content: m.content }))
      );

      const assistantMsg: Message = {
        id: (Date.now() + 1).toString(),
        sender: 'assistant',
        content: response.text,
        timestamp: new Date(),
        twinId: targetTwin.id,
      };
      setMessages(prev => [...prev, assistantMsg]);

      // Check for operational trigger keywords to start a background mission
      const triggers = ['generate', 'run', 'scan', 'analyze', 'patch', 'suppress', 'reconstruct'];
      const shouldStartJob = triggers.some(t => text.toLowerCase().includes(t));

      if (shouldStartJob) {
        const jobId = Math.random().toString(36).substr(2, 9);
        const newJob: Job = {
          id: jobId,
          twinId: targetTwin.id,
          name: `Command: ${text.substring(0, 30)}`,
          progress: 0,
          status: 'pending',
          startTime: new Date(),
          logs: [
            { id: 'init-1', timestamp: new Date(), level: 'info', message: 'Command received by Orchestrator.' },
            { id: 'init-2', timestamp: new Date(), level: 'plan', message: `Routing request to ${targetTwin.name} logic cluster.` }
          ]
        };
        
        setJobs(prev => [...prev, newJob]);
        // Hand off to the Orchestrator Service
        executeJobLifecycle(newJob, targetTwin, updateJobInState);
      }

    } catch (error) {
      console.error(error);
      const errMsg: Message = {
        id: (Date.now() + 1).toString(),
        sender: 'assistant',
        content: "Operational failure: Tactical agent rejected instruction payload.",
        timestamp: new Date(),
      };
      setMessages(prev => [...prev, errMsg]);
    } finally {
      setTwins(prev => prev.map(t => t.id === targetTwin.id ? { ...t, status: TwinStatus.IDLE } : t));
    }
  };

  const handleRunTool = (toolId: string) => {
    const tool = AVAILABLE_TOOLS.find(t => t.id === toolId);
    if (!tool) return;

    const userMsg: Message = {
      id: Date.now().toString(),
      sender: 'user',
      content: `[TOOL_CALL] Execute system tool: ${tool.name}`,
      timestamp: new Date(),
      twinId: activeTwin.id,
    };
    setMessages(prev => [...prev, userMsg]);

    const jobId = Math.random().toString(36).substr(2, 9);
    const newJob: Job = {
      id: jobId,
      twinId: activeTwin.id,
      name: `Tool: ${tool.label}`,
      progress: 0,
      status: 'pending',
      startTime: new Date(),
      logs: [
        { id: 'tool-init', timestamp: new Date(), level: 'tool', message: `Verifying permissions for ${tool.name}...` },
        { id: 'tool-auth', timestamp: new Date(), level: 'info', message: `Access granted for ${activeTwin.name} to tool capability.` }
      ]
    };

    setJobs(prev => [...prev, newJob]);
    
    // Hand off tool execution to the Orchestrator Service
    executeJobLifecycle(newJob, activeTwin, (updatedJob) => {
      updateJobInState(updatedJob);
      
      // If completed, add a chat message response
      if (updatedJob.status === 'completed') {
        const assistantMsg: Message = {
          id: Date.now().toString(),
          sender: 'assistant',
          content: `Execution of ${tool.label} complete. View the mission log for detailed analysis.`,
          timestamp: new Date(),
          twinId: activeTwin.id,
        };
        setMessages(prev => [...prev, assistantMsg]);
      }
    });
  };

  const handleSaveSettings = (updatedTwin: Twin) => {
    setTwins(prev => prev.map(t => t.id === updatedTwin.id ? updatedTwin : t));
    setView('chat');
  };

  const handleCreateTwin = (newTwin: Twin) => {
    setTwins(prev => [...prev, newTwin]);
    setActiveTwinId(newTwin.id);
    setView('chat');
    setIsCreateModalOpen(false);
  };

  const handleApprove = (id: string) => {
    setApprovals(prev => prev.map(a => a.id === id ? { ...a, status: 'approved' } : a));
    setTimeout(() => {
      setApprovals(prev => prev.filter(a => a.id !== id));
    }, 2000);
  };

  const handleDeny = (id: string) => {
    setApprovals(prev => prev.map(a => a.id === id ? { ...a, status: 'denied' } : a));
    setTimeout(() => {
      setApprovals(prev => prev.filter(a => a.id !== id));
    }, 2000);
  };

  const handleViewLogs = (jobId: string) => {
    setSelectedJobId(jobId);
    setView('logs');
  };

  const renderContent = () => {
    switch (view) {
      case 'orchestrator':
        return (
          <OrchestratorHub 
            orchestrator={orchestrator}
            messages={messages}
            onSendMessage={(txt) => handleSendMessage(txt, orchestrator)}
          />
        );
      case 'settings':
        return (
          <SettingsView 
            twin={activeTwin} 
            onSave={handleSaveSettings} 
            onCancel={() => setView('chat')}
          />
        );
      case 'logs':
        if (selectedJob) {
          const jobTwin = twins.find(t => t.id === selectedJob.twinId) || activeTwin;
          return (
            <JobLogsView 
              job={selectedJob}
              twin={jobTwin}
              onClose={() => setView('chat')}
            />
          );
        }
        return null;
      case 'search':
        return (
          <SearchView 
            messages={messages}
            jobs={jobs}
            twins={twins}
            onNavigateToChat={(twinId) => {
              setActiveTwinId(twinId);
              setView('chat');
            }}
            onNavigateToLogs={handleViewLogs}
            onClose={() => setView('orchestrator')}
          />
        );
      case 'chat':
      default:
        return (
          <ChatArea 
            messages={messages.filter(m => m.twinId === activeTwinId || m.sender === 'user')} 
            activeTwin={activeTwin}
            onSendMessage={handleSendMessage}
            onRunTool={handleRunTool}
          />
        );
    }
  };

  return (
    <div className="flex h-screen bg-[#09090b] text-zinc-100 overflow-hidden select-none">
      {isSidebarLeftOpen && (
        <SidebarLeft 
          twins={twins} 
          activeTwinId={activeTwinId} 
          currentView={view}
          onSelectTwin={(id) => {
            setActiveTwinId(id);
            setView('chat');
          }}
          onSelectOrchestrator={() => setView('orchestrator')}
          onOpenCreateModal={() => setIsCreateModalOpen(true)}
          onSelectSearch={() => setView('search')}
        />
      )}

      <main className="flex-1 flex flex-col relative min-w-0 border-x border-zinc-800/50">
        <header className="h-14 border-b border-zinc-800/50 flex items-center justify-between px-4 bg-zinc-950/50 backdrop-blur-md sticky top-0 z-10">
          <div className="flex items-center gap-3">
            <button 
              onClick={() => setIsSidebarLeftOpen(!isSidebarLeftOpen)}
              className="p-1.5 hover:bg-zinc-800 rounded-md transition-colors"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
            </button>
            <div className="flex items-center gap-2">
              <span className={`w-2 h-2 rounded-full ${view === 'orchestrator' ? 'bg-indigo-500' : view === 'logs' ? 'bg-cyan-500' : view === 'search' ? 'bg-indigo-400' : 'bg-emerald-500'} animate-pulse`}></span>
              <h1 className="font-semibold text-sm tracking-tight text-zinc-200">
                {view === 'orchestrator' ? 'Command Center' : view === 'logs' ? 'System Logs' : view === 'search' ? 'Neural Index Search' : 'Tactical Agent'}
              </h1>
            </div>
          </div>
          <div className="flex items-center gap-4">
            {view !== 'orchestrator' && view !== 'logs' && view !== 'search' && (
              <button 
                onClick={() => setView(view === 'chat' ? 'settings' : 'chat')}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${
                  view === 'settings' ? 'bg-indigo-600 text-white' : 'bg-zinc-900 text-zinc-400 hover:text-white'
                }`}
              >
                <span className="material-symbols-outlined text-sm">{view === 'chat' ? 'settings' : 'chat_bubble'}</span>
                {view === 'chat' ? 'Configure Agent' : 'Agent Chat'}
              </button>
            )}
            <button 
              onClick={() => setIsSidebarRightOpen(!isSidebarRightOpen)}
              className="p-1.5 hover:bg-zinc-800 rounded-md transition-colors"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3v18"/></svg>
            </button>
          </div>
        </header>

        {renderContent()}
      </main>

      {isSidebarRightOpen && (
        <SidebarRight 
          telemetry={telemetry}
          jobs={jobs}
          approvals={approvals}
          onApprove={handleApprove}
          onDeny={handleDeny}
          activeTwin={activeTwin}
          onViewLogs={handleViewLogs}
        />
      )}

      {isCreateModalOpen && (
        <CreateTwinModal 
          onSave={handleCreateTwin}
          onClose={() => setIsCreateModalOpen(false)}
        />
      )}
    </div>
  );
};

export default App;
