import React, { useState, useEffect, useCallback } from 'react';
import { Twin, Message, Job, Approval, TwinStatus, TelemetryData, AppView, LogEntry, TwinSettings } from './types';
import { INITIAL_TWINS, ICONS, AVAILABLE_TOOLS } from './constants';
import SidebarLeft from './components/SidebarLeft';
import SidebarRight from './components/SidebarRight';
import ChatArea from './components/ChatArea';
import SettingsView from './components/SettingsView';
import OrchestratorHub from './components/OrchestratorHub';
import IntelligenceHub from './components/IntelligenceHub';
import JobLogsView from './components/JobLogsView';
import CreateTwinModal from './components/CreateTwinModal';
import SearchView from './components/SearchView';
import CommandModal from './components/CommandModal';
import MediaControls from './components/MediaControls';
import MemoryExplorer from './pages/memory-explorer';
import Evolution from './pages/evolution';
import MediaGallery from './pages/MediaGallery';
import AgentForge from './pages/AgentForge';
import ToolForge from './pages/ToolForge';
import SystemMonitor from './components/SystemMonitor';
import OrchestratorSettings from './components/RootAdminSettings';
import FileProcessingMonitor from './components/FileProcessingMonitor';
import OAuthCallback from './pages/OAuthCallback';
import AuditDashboard from './pages/AuditDashboard';
import PhoenixGlobalSearch from './components/PhoenixGlobalSearch';
import KnowledgeAtlas from './components/KnowledgeAtlas';
import ToolInstallationProposalModal from './components/ToolInstallationProposalModal';
import { executeJobLifecycle } from './services/orchestrator';
import { usePagi } from './context/PagiContext';
import { useTelemetry } from './context/TelemetryContext';
import { useTheme } from './context/ThemeContext';
import { useDomainAttribution } from './context/DomainAttributionContext';
import { convertChatResponseToMessage } from './utils/messageConverter';
import { updateFaviconLinks } from './utils/updateFavicon';
import { ChatResponse, CompleteMessage, AgentCommand, ChatRequest } from './types/protocol';

const App: React.FC = () => {
  // Get WebSocket context
  const {
    sendChatRequest,
    messages: protocolMessages,
    isConnected,
    sessionId,
    createNewSession,
    switchToSession,
  } = usePagi();
  // Get Telemetry context
  const { telemetry, isConnected: isTelemetryConnected } = useTelemetry();
  // Get Theme context
  const { theme, toggleTheme } = useTheme();
  // Get Domain Attribution context
  const { currentAttribution } = useDomainAttribution();
  
  // Load orchestrator agent settings from localStorage on mount
  const loadOrchestratorAgentSettings = (): Partial<TwinSettings> | null => {
    try {
      const stored = localStorage.getItem('orchestrator_agent_settings');
      if (stored) {
        return JSON.parse(stored);
      }
    } catch (e) {
      console.warn('[App] Failed to load orchestrator agent settings:', e);
    }
    return null;
  };

  const initialTwins = React.useMemo(() => {
    const savedAgentSettings = loadOrchestratorAgentSettings();
    if (savedAgentSettings) {
      return INITIAL_TWINS.map(t => 
        t.id === 'twin-aegis' 
          ? { 
              ...t, 
              settings: { 
                ...t.settings, 
                ...savedAgentSettings,
              } 
            } 
          : t
      );
    }
    return INITIAL_TWINS;
  }, []);

  const [twins, setTwins] = useState<Twin[]>(initialTwins);
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
  const [isGlobalSearchOpen, setIsGlobalSearchOpen] = useState(false);
  const [isToolProposalsModalOpen, setIsToolProposalsModalOpen] = useState(false);
  
  // Check for OAuth callback on mount
  const [isOAuthCallback, setIsOAuthCallback] = useState(false);
  
  // Keyboard shortcut for global search (Ctrl+K / Cmd+K)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        setIsGlobalSearchOpen(prev => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);
  useEffect(() => {
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.get('code') || urlParams.get('error')) {
      setIsOAuthCallback(true);
    }
  }, []);

  interface Project {
    id: string;
    name: string;
    watchPath?: string; // Optional: local file system path to monitor
  }

  const DEFAULT_PROJECTS: Project[] = [
    { id: 'rapid7-siem', name: 'Rapid7 SIEM' },
    { id: 'proofpoint', name: 'Proofpoint' },
    { id: 'sentinelone', name: 'SentinelOne' },
    { id: 'zscaler', name: 'Zscaler' },
    { id: 'crowdstrike', name: 'CrowdStrike' },
    { id: 'microsoft-defender', name: 'Microsoft Defender' },
    { id: 'splunk', name: 'Splunk' },
    { id: 'qualys', name: 'Qualys' },
    { id: 'tenable', name: 'Tenable' },
    { id: 'palo-alto', name: 'Palo Alto Networks' },
    { id: 'fortinet', name: 'Fortinet' },
    { id: 'cisco-umbrella', name: 'Cisco Umbrella' },
  ];

  const [projects, setProjects] = useState<Project[]>(() => {
    try {
      const raw = localStorage.getItem('pagi_projects');
      if (!raw) return DEFAULT_PROJECTS;
      const parsed = JSON.parse(raw) as unknown;
      if (!Array.isArray(parsed)) return DEFAULT_PROJECTS;
      const sanitized: Project[] = parsed
        .filter((p: any) => p && typeof p.id === 'string' && typeof p.name === 'string')
        .map((p: any) => ({ 
          id: String(p.id), 
          name: String(p.name),
          watchPath: typeof p.watchPath === 'string' ? String(p.watchPath) : undefined
        }));
      return sanitized.length > 0 ? sanitized : DEFAULT_PROJECTS;
    } catch {
      return DEFAULT_PROJECTS;
    }
  });

  const [activeProjectId, setActiveProjectId] = useState<string | null>(() => {
    try {
      return localStorage.getItem('pagi_active_project_id');
    } catch {
      return null;
    }
  });

  // Persist active project
  useEffect(() => {
    try {
      if (activeProjectId) localStorage.setItem('pagi_active_project_id', activeProjectId);
      else localStorage.removeItem('pagi_active_project_id');
    } catch {
      // ignore
    }
  }, [activeProjectId]);

  type ProjectSessionMap = Record<string, string>; // projectId -> sessionId

  const loadProjectSessionMap = useCallback((): ProjectSessionMap => {
    try {
      const raw = localStorage.getItem('pagi_project_sessions');
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== 'object') return {};
      const out: ProjectSessionMap = {};
      for (const [k, v] of Object.entries(parsed)) {
        if (typeof k === 'string' && typeof v === 'string' && v.trim()) out[k] = v;
      }
      return out;
    } catch {
      return {};
    }
  }, []);

  const saveProjectSessionMap = useCallback((m: ProjectSessionMap) => {
    try {
      localStorage.setItem('pagi_project_sessions', JSON.stringify(m));
    } catch {
      // ignore
    }
  }, []);

  const normalizeProjectName = useCallback((name: string) => name.trim().replace(/\s+/g, ' '), []);

  const ensureProjectByName = useCallback((projectNameRaw: string, projectIdHint?: string): Project => {
    const projectName = normalizeProjectName(projectNameRaw);
    const existingById = projectIdHint ? projects.find(p => p.id === projectIdHint) : undefined;
    if (existingById) return existingById;

    const existingByName = projects.find(p => p.name.toLowerCase() === projectName.toLowerCase());
    if (existingByName) return existingByName;

    const id = projectIdHint && projectIdHint.trim().length > 0 ? projectIdHint.trim() : `project-${crypto.randomUUID()}`;
    const created = { id, name: projectName };
    setProjects(prev => [...prev, created]);
    return created;
  }, [normalizeProjectName, projects]);

  // Update favicon links on mount
  useEffect(() => {
    updateFaviconLinks();
  }, []);

  // Listen for orchestrator agent settings changes from OrchestratorSettings
  useEffect(() => {
    const handleAgentSettingsChange = (event: CustomEvent) => {
      const newSettings = event.detail.settings;
      setTwins(prev => prev.map(t => 
        t.id === 'twin-aegis' 
          ? { 
              ...t, 
              settings: { 
                ...t.settings, 
                temperature: newSettings.temperature,
                topP: newSettings.topP,
                maxMemory: newSettings.maxMemory,
                tokenLimit: newSettings.tokenLimit,
              } 
            } 
          : t
      ));
    };

    window.addEventListener('orchestratorAgentSettingsChanged', handleAgentSettingsChange as EventListener);
    return () => {
      window.removeEventListener('orchestratorAgentSettingsChanged', handleAgentSettingsChange as EventListener);
    };
  }, []);

  // Persist projects list
  useEffect(() => {
    try {
      localStorage.setItem('pagi_projects', JSON.stringify(projects));
    } catch {
      // ignore (storage full / blocked)
    }
  }, [projects]);

  // Notify orchestrator when gallery is open (for context awareness)
  useEffect(() => {
    // Set a flag that the orchestrator can check via media_active or a separate mechanism
    // For now, we'll use a localStorage flag that can be checked
    if (view === 'gallery') {
      localStorage.setItem('pagi_gallery_active', 'true');
    } else {
      localStorage.removeItem('pagi_gallery_active');
    }
  }, [view]);

  // Prevent re-processing the same issued_command on every render.
  // `protocolMessages` is an append-only stream, so without this, the modal
  // re-opens repeatedly for the same command.
  const handledIssuedCommandIdsRef = React.useRef<Set<string>>(new Set());

  // When switching sessions, clear the in-memory UI transcript and reset command de-dupe.
  useEffect(() => {
    setMessages([]);
    handledIssuedCommandIdsRef.current = new Set();
  }, [sessionId]);

  const activeTwin = twins.find(t => t.id === activeTwinId) || twins[0];
  const orchestrator = twins.find(t => t.isOrchestrator) || twins[0];
  const selectedJob = jobs.find(j => j.id === selectedJobId);


  // Handle agent commands from backend
  const handleAgentCommand = useCallback((command: CompleteMessage['issued_command'], response: CompleteMessage) => {
    if (!command) return;

    console.log('[App] Agent command received:', command);

    switch (command.command) {
      case 'create_project_chat': {
        const project = ensureProjectByName(command.project_name, command.project_id);
        setActiveProjectId(project.id);
        setView('orchestrator');

        const newSessionId = createNewSession();
        const map = loadProjectSessionMap();
        map[project.id] = newSessionId;
        saveProjectSessionMap(map);
        break;
      }

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

      case 'crew_list':
        // Navigate to orchestrator view to show crew list
        setView('orchestrator');
        // The CrewList component will automatically refresh and show the new agent if agent_id is provided
        break;
    }
  }, [createNewSession, ensureProjectByName, loadProjectSessionMap, saveProjectSessionMap]);


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
    let responseMessage = '';
    switch (command.command) {
      case 'prompt_for_config':
        responseMessage = `[CONFIG_RESPONSE] ${command.config_key}: ${value}`;
        break;
      case 'execute_tool':
        responseMessage = `[TOOL_EXECUTED] ${command.tool_name} - ${value}`;
        break;
      case 'show_memory_page':
        responseMessage = `[MEMORY_SHOWN] ${command.memory_id}`;
        break;
      case 'create_project_chat':
        // This command is handled immediately on receipt and should not surface in the modal.
        return;
    }

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
    let denialMessage = '';
    switch (activeCommand.command) {
      case 'prompt_for_config':
        denialMessage = `[CONFIG_DENIED] ${activeCommand.config_key}`;
        break;
      case 'execute_tool':
        denialMessage = `[TOOL_DENIED] ${activeCommand.tool_name}`;
        break;
      case 'show_memory_page':
        denialMessage = `[MEMORY_DENIED] ${activeCommand.memory_id}`;
        break;
      case 'create_project_chat':
        // Should not be deniable.
        return;
    }

    sendChatRequest(denialMessage, undefined); // Denial messages don't need settings

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

    // Send message via WebSocket with twin settings
    if (isConnected) {
      sendChatRequest(text, {
        temperature: targetTwin.settings.temperature,
        top_p: targetTwin.settings.topP,
        max_tokens: targetTwin.settings.tokenLimit * 1000, // Convert from K to actual tokens
        max_memory: targetTwin.settings.maxMemory,
      });
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
      case 'intelligence-hub':
        return (
          <IntelligenceHub
            orchestrator={orchestrator}
          />
        );
      case 'phoenix':
        // Back-compat: older navigation target now renders the Intelligence Hub.
        return (
          <IntelligenceHub
            orchestrator={orchestrator}
          />
        );
      case 'agent-forge':
        return (
          <AgentForge
            onClose={() => setView('orchestrator')}
          />
        );
      case 'tool-forge':
        return (
          <ToolForge
            onClose={() => setView('orchestrator')}
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
      case 'gallery':
        return (
          <MediaGallery 
            onClose={() => setView('orchestrator')}
          />
        );
      case 'system-status':
        return (
          <SystemMonitor 
            twinId={orchestrator.id}
            onClose={() => setView('orchestrator')}
          />
        );
      case 'file-processing-monitor':
        return (
          <FileProcessingMonitor 
            projectId={activeProjectId || undefined}
            onClose={() => setView('orchestrator')}
          />
        );
      case 'orchestrator-settings':
        return (
          <OrchestratorSettings 
            onClose={() => setView('orchestrator')}
          />
        );
      case 'audit':
        return (
          <AuditDashboard 
            onClose={() => setView('orchestrator')}
          />
        );
      case 'knowledge-atlas':
        return (
          <div className="w-full h-full">
            <KnowledgeAtlas 
              onNodeClick={(result) => {
                // Open search result in Phoenix Global Search or show details
                console.log('Knowledge node clicked:', result);
              }}
            />
          </div>
        );
      case 'chat':
      default:
        return (
          <ChatArea 
            messages={messages.filter(m => m.twinId === activeTwinId || m.sender === 'user')} 
            activeTwin={activeTwin}
            onSendMessage={handleSendMessage}
            onRunTool={handleRunTool}
            onOpenSettings={() => setView('settings')}
          />
        );
    }
  };

  // Show OAuth callback page if we're handling OAuth redirect
  if (isOAuthCallback) {
    return <OAuthCallback />;
  }

  return (
    <div className="flex h-screen bg-[var(--bg-primary)] text-[var(--text-primary)] overflow-hidden">
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
          onSelectIntelligenceHub={() => setView('intelligence-hub')}
          onOpenCreateModal={() => setIsCreateModalOpen(true)}
          onSelectSearch={() => setView('search')}
          onSelectMemoryExplorer={() => setView('memory-explorer')}
          onSelectEvolution={() => setView('evolution')}
          onSelectSystemStatus={() => setView('system-status')}
          onSelectPhoenix={() => setView('phoenix')}
          onSelectFileProcessingMonitor={() => setView('file-processing-monitor')}
          onSelectAgentForge={() => setView('agent-forge')}
          onSelectToolForge={() => setView('tool-forge')}
          onSelectAudit={() => setView('audit')}
          onSelectKnowledgeAtlas={() => setView('knowledge-atlas')}
          onOpenToolProposals={() => setIsToolProposalsModalOpen(true)}
          projects={projects}
          onSelectProject={(projectId) => {
            // Navigate to orchestrator view for project context and switch to the project's session.
            setView('orchestrator');
            setActiveProjectId(projectId);

            const map = loadProjectSessionMap();
            const projectSession = map[projectId];
            if (projectSession) {
              switchToSession(projectSession);
            } else {
              const newSessionId = createNewSession();
              map[projectId] = newSessionId;
              saveProjectSessionMap(map);
            }
          }}
          onCreateProject={(name) => {
            const trimmed = name.trim();
            if (!trimmed) return;
            const id = `project-${crypto.randomUUID()}`;
            setProjects((prev) => [...prev, { id, name: trimmed }]);
          }}
          onRenameProject={(projectId, name) => {
            const trimmed = name.trim();
            if (!trimmed) return;
            setProjects((prev) => prev.map((p) => (p.id === projectId ? { ...p, name: trimmed } : p)));
          }}
          onDeleteProject={(projectId) => {
            setProjects((prev) => prev.filter((p) => p.id !== projectId));
          }}
          onConfigureWatchPath={async (projectId, watchPath) => {
            const project = projects.find((p) => p.id === projectId);
            if (!project) return;
            
            // Update local state
            setProjects((prev) => prev.map((p) => 
              p.id === projectId ? { ...p, watchPath: watchPath || undefined } : p
            ));
            
            // If watch path is provided, configure it on the backend
            if (watchPath.trim()) {
              try {
                const { configureProjectWatch } = await import('./services/projectService');
                await configureProjectWatch(projectId, project.name, watchPath.trim());
              } catch (error) {
                console.error('[App] Failed to configure watch path:', error);
                // Revert on error
                setProjects((prev) => prev.map((p) => 
                  p.id === projectId ? { ...p, watchPath: project.watchPath } : p
                ));
                window.alert(`Failed to configure watch path: ${error instanceof Error ? error.message : String(error)}`);
              }
            }
          }}
          onSelectOrchestratorSettings={() => setView('orchestrator-settings')}
        />
      )}

      <main className="flex-1 flex flex-col relative min-w-0 border-x border-[rgb(var(--bg-steel-rgb)/0.3)]">
        <header className="h-14 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center justify-between px-4 bg-[var(--bg-secondary)] sticky top-0 z-10">
          <div className="flex items-center gap-3">
            <button 
              onClick={() => setIsSidebarLeftOpen(!isSidebarLeftOpen)}
              className="p-1.5 hover:bg-[var(--bg-muted)] rounded-md transition-colors"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
            </button>
            <div className="flex items-center gap-2">
              <span className={`w-2 h-2 rounded-full ${view === 'orchestrator' ? 'bg-[var(--bg-steel)]' : view === 'logs' ? 'bg-[var(--bg-muted)]' : view === 'search' ? 'bg-[var(--bg-steel)]' : 'bg-[var(--bg-muted)]'} animate-pulse`}></span>
              <h1 className="font-semibold text-sm tracking-tight text-[var(--text-primary)]">
                {view === 'orchestrator'
                  ? 'Command Center'
                  : view === 'intelligence-hub'
                  ? 'Intelligence Hub'
                  : view === 'phoenix'
                  ? 'Intelligence Hub'
                  : view === 'logs'
                  ? 'System Logs'
                  : view === 'search'
                  ? 'Neural Index Search'
                  : view === 'memory-explorer'
                  ? 'Neural Archive Explorer'
                  : view === 'evolution'
                  ? 'Evolutionary Timeline'
                  : view === 'gallery'
                  ? 'Neural Archive'
                  : view === 'system-status'
                  ? 'System Status'
                  : view === 'orchestrator-settings'
                  ? 'Orchestrator Settings'
                  : view === 'file-processing-monitor'
                  ? 'File Processing Monitor'
                  : view === 'agent-forge'
                  ? 'AgentForge - Visual Agent Builder'
                  : view === 'tool-forge'
                  ? 'ToolForge - Dynamic Tool Development'
                  : view === 'audit'
                  ? 'Phoenix Audit Dashboard'
                  : 'Tactical Agent'}
              </h1>
            </div>
          </div>
          <div className="flex items-center gap-4">
            {(view === 'chat' || view === 'settings') && (
              <button
                onClick={() => setView(view === 'chat' ? 'settings' : 'chat')}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-bold transition-all ${
                  view === 'settings' ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]' : 'bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[var(--bg-steel)] hover:text-[var(--text-on-accent)]'
                }`}
              >
                <span className="material-symbols-outlined text-sm">{view === 'chat' ? 'settings' : 'chat_bubble'}</span>
                {view === 'chat' ? 'Configure Agent' : 'Agent Chat'}
              </button>
            )}

            {/* Audio/Video/Recording controls moved into the top-right title panel */}
            <MediaControls
              placement="header"
              onOpenGallery={() => {
                console.log('[App] Opening Neural Archive (gallery view)');
                setView('gallery');
              }}
            />

            {/* Dark Mode Toggle */}
            <button
              onClick={toggleTheme}
              className="p-1.5 hover:bg-[var(--bg-muted)] rounded-md transition-colors"
              title={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
            >
              {theme === 'light' ? (
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>
              ) : (
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>
              )}
            </button>

            <button
              onClick={() => setIsSidebarRightOpen(!isSidebarRightOpen)}
              className="p-1.5 hover:bg-[var(--bg-muted)] rounded-md transition-colors"
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
          domainAttribution={currentAttribution}
        />
      )}

      {isCreateModalOpen && (
        <CreateTwinModal 
          onSave={handleCreateTwin}
          onClose={() => setIsCreateModalOpen(false)}
        />
      )}

      {/* Phoenix Global Search */}
      <PhoenixGlobalSearch
        isOpen={isGlobalSearchOpen}
        onClose={() => setIsGlobalSearchOpen(false)}
        sessionId={sessionId}
        onNavigateToChat={(twinId) => {
          setActiveTwinId(twinId);
          setView('chat');
          setIsGlobalSearchOpen(false);
        }}
        onNavigateToMemory={(namespace) => {
          setView('search');
          setIsGlobalSearchOpen(false);
        }}
      />

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

      {/* Tool Installation Proposal Modal */}
      {isToolProposalsModalOpen && (
        <ToolInstallationProposalModal
          isOpen={isToolProposalsModalOpen}
          onClose={() => setIsToolProposalsModalOpen(false)}
          onProposalUpdated={() => {
            // Refresh notification count will be handled by SidebarLeft's useEffect
          }}
        />
      )}

      {/* MediaControls moved into header */}
    </div>
  );
};

export default App;
