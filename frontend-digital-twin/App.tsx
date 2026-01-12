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
import CommandModal from './components/CommandModal';
import MediaControls from './components/MediaControls';
import MemoryExplorer from './pages/memory-explorer';
import Evolution from './pages/evolution';
import { executeJobLifecycle } from './services/orchestrator';
import { usePagi } from './context/PagiContext';
import { useTelemetry } from './context/TelemetryContext';
import { convertChatResponseToMessage } from './utils/messageConverter';
import { ChatResponse, CompleteMessage, AgentCommand, ChatRequest } from './types/protocol';

const App: React.FC = () => {
  // Get WebSocket context
  const { sendChatRequest, messages: protocolMessages, isConnected } = usePagi();
  // Get Telemetry context
  const { telemetry, isConnected: isTelemetryConnected } = useTelemetry();
  
  const [twins, setTwins] = useState<Twin[]>(INITIAL_TWINS);
  const [activeTwinId, setActiveTwinId] = useState<string>(INITIAL_TWINS[0].id);
  const [view, setView] = useState<AppView>('orchestrator');
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [approvals, setApprovals] = useState<Approval[]>([]);
  const [isSidebarLeftOpen, setIsSidebarLeftOpen] = useState(true);
  const [isSidebarRightOpen, setIsSidebarRightOpen] = useState(true);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [activeCommand, setActiveCommand] = useState<AgentCommand | null>(null);
  const [commandMessageId, setCommandMessageId] = useState<string | null>(null);
  const [activeDecisionTrace, setActiveDecisionTrace] = useState<string | null>(null);

  // Prevent re-processing the same issued_command on every render.
  // `protocolMessages` is an append-only stream, so without this, the modal
  // re-opens repeatedly for the same command.
  const handledIssuedCommandIdsRef = React.useRef<Set<string>>(new Set());

  const activeTwin = twins.find(t => t.id === activeTwinId) || twins[0];
  const orchestrator = twins.find(t => t.isOrchestrator) || twins[0];
  const selectedJob = jobs.find(j => j.id === selectedJobId);


  // Handle agent commands from backend
  const handleAgentCommand = useCallback((command: CompleteMessage['issued_command'], response: CompleteMessage) => {
    if (!command) return;

    console.log('[App] Agent command received:', command);

    switch (command.command) {
      case 'show_memory_page':
        // Show modal for memory page request
        setActiveCommand(command);
        setCommandMessageId(response.id);
        setActiveDecisionTrace(response.raw_orchestrator_decision ?? null);
        break;

      case 'prompt_for_config':
        // Show configuration prompt modal
        setActiveCommand(command);
        setCommandMessageId(response.id);
        setActiveDecisionTrace(response.raw_orchestrator_decision ?? null);
        break;

      case 'execute_tool':
        // Show tool execution authorization modal
        setActiveCommand(command);
        setCommandMessageId(response.id);
        setActiveDecisionTrace(response.raw_orchestrator_decision ?? null);
        break;
    }
  }, []);


  // Convert protocol messages to UI messages
  useEffect(() => {
    const convertedMessages: Message[] = [];
    const processedIds = new Set<string>();

    protocolMessages.forEach((protocolMsg) => {
      // Only some protocol message variants have a stable `id`.
      // De-dupe complete/chunk messages by id; always process status updates.
      const msgId = (protocolMsg.type === 'complete_message' || protocolMsg.type === 'message_chunk')
        ? protocolMsg.id
        : null;

      if (msgId && processedIds.has(msgId)) {
        return;
      }

      const uiMessage = convertChatResponseToMessage(protocolMsg, activeTwinId);
      if (uiMessage) {
        convertedMessages.push(uiMessage);
        if (msgId) {
          processedIds.add(msgId);
        }

        // Update twin status based on message type
        if (protocolMsg.type === 'complete_message') {
          // Set twin to IDLE when complete message arrives
          setTwins(prev => prev.map(t => t.id === activeTwinId ? { ...t, status: TwinStatus.IDLE } : t));
          
          // Handle agent commands from complete messages
          if (protocolMsg.issued_command && !handledIssuedCommandIdsRef.current.has(protocolMsg.id)) {
            handledIssuedCommandIdsRef.current.add(protocolMsg.id);
            handleAgentCommand(protocolMsg.issued_command, protocolMsg);
          }
        } else if (protocolMsg.type === 'status_update' && protocolMsg.status === 'busy') {
          // Set twin to THINKING when status is busy
          setTwins(prev => prev.map(t => t.id === activeTwinId ? { ...t, status: TwinStatus.THINKING } : t));
        }
      }
    });

    // Merge with existing messages, avoiding duplicates
    setMessages(prev => {
      const existingIds = new Set(prev.map(m => m.id));
      const newMessages = convertedMessages.filter(m => !existingIds.has(m.id));
      return [...prev, ...newMessages];
    });
  }, [protocolMessages, activeTwinId, handleAgentCommand]);

  // Telemetry is now provided by TelemetryContext via SSE
  // No need for local state or polling

  // Centralized job updater to avoid race conditions in state updates
  const updateJobInState = useCallback((updatedJob: Job) => {
    setJobs(prev => prev.map(j => j.id === updatedJob.id ? updatedJob : j));
  }, []);

  // Handle command execution (user approved)
  const handleCommandExecute = useCallback((command: AgentCommand, value: string) => {
    if (!commandMessageId || !sendChatRequest) {
      console.error('[App] Cannot send command response: missing message ID or client');
      return;
    }

    console.log('[App] Command executed:', command.command, 'value:', value);

    // Send response back to backend
    // Format: Send a follow-up message indicating the command was executed
    const responseMessage = command.command === 'prompt_for_config'
      ? `[CONFIG_RESPONSE] ${command.config_key}: ${value}`
      : command.command === 'execute_tool'
      ? `[TOOL_EXECUTED] ${command.tool_name} - ${value}`
      : `[MEMORY_SHOWN] ${command.memory_id}`;

    sendChatRequest(responseMessage);

    // Handle tool execution locally if needed
    if (command.command === 'execute_tool') {
      // Check for operational trigger keywords to start a background mission
      const triggers = ['generate', 'run', 'scan', 'analyze', 'patch', 'suppress', 'reconstruct'];
      const shouldStartJob = triggers.some(t => 
        command.tool_name.toLowerCase().includes(t) || 
        (typeof command.arguments === 'string' && command.arguments.toLowerCase().includes(t))
      );

      if (shouldStartJob) {
        const jobId = Math.random().toString(36).substr(2, 9);
        const newJob: Job = {
          id: jobId,
          twinId: activeTwinId,
          name: `Command: ${command.tool_name}`,
          progress: 0,
          status: 'pending',
          startTime: new Date(),
          logs: [
            { id: 'init-1', timestamp: new Date(), level: 'info', message: 'Command received by Orchestrator.' },
            { id: 'init-2', timestamp: new Date(), level: 'plan', message: `Routing request to ${activeTwin.name} logic cluster.` }
          ]
        };
        
        setJobs(prev => [...prev, newJob]);
        executeJobLifecycle(newJob, activeTwin, updateJobInState);
      }
    } else if (command.command === 'show_memory_page') {
      // Navigate to search view with the memory query
      setView('search');
      // TODO: Pre-populate search query with command.query
    }

    // Clear command state
    setActiveCommand(null);
    setCommandMessageId(null);
    setActiveDecisionTrace(null);
  }, [commandMessageId, sendChatRequest, activeTwinId, activeTwin, updateJobInState]);

  // Handle command denial (user rejected)
  const handleCommandDeny = useCallback(() => {
    if (!commandMessageId || !sendChatRequest || !activeCommand) {
      console.error('[App] Cannot send command denial: missing message ID, client, or command');
      return;
    }

    console.log('[App] Command denied:', activeCommand.command);

    // Send denial response back to backend
    const denialMessage = activeCommand.command === 'prompt_for_config'
      ? `[CONFIG_DENIED] ${activeCommand.config_key}`
      : activeCommand.command === 'execute_tool'
      ? `[TOOL_DENIED] ${activeCommand.tool_name}`
      : `[MEMORY_DENIED] ${activeCommand.memory_id}`;

    sendChatRequest(denialMessage);

    // Clear command state
    setActiveCommand(null);
    setCommandMessageId(null);
    setActiveDecisionTrace(null);
  }, [commandMessageId, activeCommand, sendChatRequest]);

  const handleSendMessage = useCallback((text: string, twinOverride?: Twin) => {
    const targetTwin = twinOverride || activeTwin;
    
    // Add user message to UI immediately
    const userMsg: Message = {
      id: `user-${Date.now()}`,
      sender: 'user',
      content: text,
      timestamp: new Date(),
      twinId: targetTwin.id,
    };
    setMessages(prev => [...prev, userMsg]);

    // Update twin status to thinking
    setTwins(prev => prev.map(t => t.id === targetTwin.id ? { ...t, status: TwinStatus.THINKING } : t));

    // Send message via WebSocket
    if (isConnected) {
      sendChatRequest(text);
    } else {
      // Show error if not connected
      const errMsg: Message = {
        id: `error-${Date.now()}`,
        sender: 'assistant',
        content: "Connection not ready. Please wait for the Digital Twin to connect...",
        timestamp: new Date(),
        twinId: targetTwin.id,
      };
      setMessages(prev => [...prev, errMsg]);
    }
  }, [activeTwin, isConnected, sendChatRequest]);

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
      case 'evolution':
        return (
          <Evolution
            onClose={() => setView('orchestrator')}
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
      case 'memory-explorer':
        return (
          <MemoryExplorer 
            activeTwin={activeTwin}
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
    <div className="flex h-screen bg-[#9EC9D9] text-[#0b1b2b] overflow-hidden">
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
          onSelectMemoryExplorer={() => setView('memory-explorer')}
          onSelectEvolution={() => setView('evolution')}
        />
      )}

      <main className="flex-1 flex flex-col relative min-w-0 border-x border-[#5381A5]/30">
        <header className="h-14 border-b border-[#5381A5]/30 flex items-center justify-between px-4 bg-[#90C3EA] sticky top-0 z-10">
          <div className="flex items-center gap-3">
            <button 
              onClick={() => setIsSidebarLeftOpen(!isSidebarLeftOpen)}
              className="p-1.5 hover:bg-[#78A2C2] rounded-md transition-colors"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
            </button>
            <div className="flex items-center gap-2">
              <span className={`w-2 h-2 rounded-full ${view === 'orchestrator' ? 'bg-[#5381A5]' : view === 'logs' ? 'bg-[#78A2C2]' : view === 'search' ? 'bg-[#5381A5]' : 'bg-[#78A2C2]'} animate-pulse`}></span>
              <h1 className="font-semibold text-sm tracking-tight text-[#0b1b2b]">
                {view === 'orchestrator'
                  ? 'Command Center'
                  : view === 'logs'
                  ? 'System Logs'
                  : view === 'search'
                  ? 'Neural Index Search'
                  : view === 'memory-explorer'
                  ? 'Neural Archive Explorer'
                  : view === 'evolution'
                  ? 'Evolutionary Timeline'
                  : 'Tactical Agent'}
              </h1>
            </div>
          </div>
          <div className="flex items-center gap-4">
            {(view === 'chat' || view === 'settings') && (
              <button 
                onClick={() => setView(view === 'chat' ? 'settings' : 'chat')}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${
                  view === 'settings' ? 'bg-[#5381A5] text-white' : 'bg-[#78A2C2] text-[#0b1b2b] hover:bg-[#5381A5] hover:text-white'
                }`}
              >
                <span className="material-symbols-outlined text-sm">{view === 'chat' ? 'settings' : 'chat_bubble'}</span>
                {view === 'chat' ? 'Configure Agent' : 'Agent Chat'}
              </button>
            )}
            <button 
              onClick={() => setIsSidebarRightOpen(!isSidebarRightOpen)}
              className="p-1.5 hover:bg-[#78A2C2] rounded-md transition-colors"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3v18"/></svg>
            </button>
          </div>
        </header>

        {renderContent()}
      </main>

      {isSidebarRightOpen && (
        <SidebarRight 
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

      {/* Agent Command Modal */}
      <CommandModal
        command={activeCommand}
        decisionTrace={activeDecisionTrace}
        isVisible={activeCommand !== null}
        onClose={() => {
          setActiveCommand(null);
          setCommandMessageId(null);
          setActiveDecisionTrace(null);
        }}
        onExecute={handleCommandExecute}
        onDeny={handleCommandDeny}
      />

      <MediaControls />
    </div>
  );
};

export default App;
