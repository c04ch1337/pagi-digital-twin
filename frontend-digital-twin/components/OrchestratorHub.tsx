
import React, { useState, useRef, useEffect } from 'react';
import { Message, Twin, TwinStatus } from '../types';
import { useSpeechToText } from '../hooks/useSpeechToText';
import CrewList from './CrewList';
import TrustNetworkMap from './TrustNetworkMap';
import PhoenixMemoryFlow from './PhoenixMemoryFlow';

interface OrchestratorHubProps {
  orchestrator: Twin;
  messages: Message[];
  onSendMessage: (text: string) => void;
  initialTab?: 'chat' | 'intelligence';
}

const OrchestratorHub: React.FC<OrchestratorHubProps> = ({ orchestrator, messages, onSendMessage, initialTab = 'chat' }) => {
  const [input, setInput] = useState('');
  const [activeTab, setActiveTab] = useState<'chat' | 'intelligence'>(initialTab);
  const [intelligenceView, setIntelligenceView] = useState<'network' | 'memory'>('network');
  const scrollRef = useRef<HTMLDivElement>(null);
  
  // Intelligence Tab Activity Indicator State
  const [recentTopics, setRecentTopics] = useState<string[]>([]);
  const [hasActiveMemoryTransfer, setHasActiveMemoryTransfer] = useState(false);
  const [hasActiveConsensusSession, setHasActiveConsensusSession] = useState(false);
  const [showTooltip, setShowTooltip] = useState(false);
  const eventSourceRef = useRef<EventSource | null>(null);

  const stt = useSpeechToText({ lang: 'en-US', continuous: false, interimResults: true });
  const dictationBaseRef = useRef<string>('');
  const lastAppliedDictationRef = useRef<string>('');

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  // Connect to SSE stream for Intelligence Tab activity indicator
  useEffect(() => {
    const sseUrl = '/api/phoenix/stream';
    const eventSource = new EventSource(sseUrl);
    eventSourceRef.current = eventSource;

    eventSource.addEventListener('memory_transfer', (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data);
        if (data.topic) {
          // Update recent topics (keep last 3)
          setRecentTopics(prev => {
            const updated = [data.topic, ...prev.filter(t => t !== data.topic)].slice(0, 3);
            return updated;
          });
          
          // Trigger pulse animation
          setHasActiveMemoryTransfer(true);
          setTimeout(() => setHasActiveMemoryTransfer(false), 2000);
        }
      } catch (error) {
        console.error('[OrchestratorHub] Failed to parse memory transfer event:', error);
      }
    });

    eventSource.addEventListener('consensus_vote', () => {
      // When we receive a consensus vote, there's likely an active session
      setHasActiveConsensusSession(true);
    });

    eventSource.addEventListener('consensus_result', () => {
      // When consensus result is received, session is likely complete
      // But we'll keep checking via polling
    });

    eventSource.onerror = () => {
      console.error('[OrchestratorHub] SSE connection error');
    };

    return () => {
      eventSource.close();
      eventSourceRef.current = null;
    };
  }, []);

  // Poll for active consensus sessions by checking recent consensus events
  useEffect(() => {
    const checkConsensusSessions = async () => {
      // Since we don't have a direct endpoint for all active sessions,
      // we'll rely on the consensus_vote events from SSE to indicate activity
      // The state is managed by the event listeners above
    };

    const interval = setInterval(checkConsensusSessions, 15000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    if (!stt.state.isListening) return;
    const base = dictationBaseRef.current;
    const parts = [base, stt.state.finalText, stt.state.interimText].filter(Boolean);
    const next = parts.join(' ').replace(/\s+/g, ' ').trimStart();
    if (lastAppliedDictationRef.current === next) return;
    lastAppliedDictationRef.current = next;
    setInput(next);
  }, [stt.state.finalText, stt.state.interimText, stt.state.isListening]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (input.trim()) {
      onSendMessage(input.trim());
      setInput('');
    }
  };

  return (
    <div className="flex-1 flex flex-col bg-[var(--bg-primary)] overflow-hidden relative">
      {/* Background Tactical Grid */}
      <div className="absolute inset-0 opacity-[0.03] pointer-events-none" 
           style={{ backgroundImage: 'linear-gradient(rgb(var(--bg-steel-rgb)) 1px, transparent 1px), linear-gradient(90deg, rgb(var(--bg-steel-rgb)) 1px, transparent 1px)', backgroundSize: '40px 40px' }} />

      {/* Tab Navigation */}
      <div className="flex border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <button
          onClick={() => setActiveTab('chat')}
          className={`flex-1 px-4 py-2 text-xs font-bold transition-all border-b-2 ${
            activeTab === 'chat'
              ? 'border-[var(--bg-steel)] text-[var(--text-secondary)] bg-[var(--bg-muted)]'
              : 'border-transparent text-[rgb(var(--text-secondary-rgb)/0.6)] hover:text-[var(--text-secondary)] hover:bg-[rgb(var(--bg-muted-rgb)/0.5)]'
          }`}
        >
          <span className="material-symbols-outlined text-sm align-middle mr-1">chat</span>
          Command Chat
        </button>
        <button
          onClick={() => setActiveTab('intelligence')}
          onMouseEnter={() => setShowTooltip(true)}
          onMouseLeave={() => setShowTooltip(false)}
          className={`flex-1 px-4 py-2 text-xs font-bold transition-all border-b-2 relative ${
            activeTab === 'intelligence'
              ? 'border-[var(--bg-steel)] text-[var(--text-secondary)] bg-[var(--bg-muted)]'
              : 'border-transparent text-[rgb(var(--text-secondary-rgb)/0.6)] hover:text-[var(--text-secondary)] hover:bg-[rgb(var(--bg-muted-rgb)/0.5)]'
          }`}
        >
          <span className="material-symbols-outlined text-sm align-middle mr-1">psychology</span>
          Intelligence
          
          {/* Activity Pulse Indicator */}
          {hasActiveMemoryTransfer && (
            <span 
              className="absolute top-1 right-1 w-2 h-2 rounded-full bg-[var(--success)]"
              style={{
                animation: 'pulse-glow 2s ease-in-out infinite',
                boxShadow: '0 0 8px rgba(var(--success-rgb), 0.8)',
              }}
            />
          )}
          
          {/* Consensus Session Checkmark - positioned differently if pulse is active */}
          {hasActiveConsensusSession && (
            <span 
              className={`absolute ${hasActiveMemoryTransfer ? 'top-1 right-4' : 'top-1 right-1'} w-2 h-2 rounded-full bg-[var(--bg-steel)] flex items-center justify-center`}
              style={{
                boxShadow: '0 0 6px rgba(var(--bg-steel-rgb), 0.6)',
              }}
              title="Active consensus session awaiting review"
            >
              <span className="material-symbols-outlined text-[8px] text-[var(--text-on-accent)]">check</span>
            </span>
          )}
          
          {/* Tooltip */}
          {showTooltip && (recentTopics.length > 0 || hasActiveConsensusSession) && (
            <div className="absolute bottom-full left-1/2 transform -translate-x-1/2 mb-2 px-3 py-2 bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.5)] rounded-lg shadow-lg z-50 min-w-[200px]">
              {recentTopics.length > 0 && (
                <div className="text-[10px] text-[var(--text-secondary)] mb-1.5">
                  <div className="font-bold uppercase mb-1">Recent Topics:</div>
                  <div className="space-y-0.5">
                    {recentTopics.map((topic, idx) => (
                      <div key={idx} className="font-mono text-[var(--text-primary)] truncate">
                        {topic}
                      </div>
                    ))}
                  </div>
                </div>
              )}
              {hasActiveConsensusSession && (
                <div className="text-[10px] text-[var(--bg-steel)] mt-1.5 pt-1.5 border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
                  <span className="material-symbols-outlined text-xs align-middle mr-1">verified</span>
                  Consensus session active
                </div>
              )}
            </div>
          )}
        </button>
      </div>

      <div className="flex-1 flex flex-col md:flex-row min-h-0 gap-4">
        {activeTab === 'chat' ? (
          <>
            {/* Unified Command Chat */}
            <div className="flex-1 flex flex-col min-w-0">
              <div className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center justify-between bg-[var(--bg-secondary)]">
                 <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-[var(--bg-steel)]">terminal</span>
                    <span className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)]">Powered by Phoenix AGI (PAGI OS v0.1)</span>
                 </div>
                 <div className="text-[9px] text-[var(--text-secondary)] font-mono">TRANSPORT: Unencrypted (Dev Mode)</div>
              </div>
          
          <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-4">
            {messages.filter(m => m.twinId === orchestrator.id || m.sender === 'user').map(msg => {
              const isError = msg.content.includes('[ERROR]') || msg.content.includes('Connection Error') || msg.content.includes('Network Error');
              return (
                <div key={msg.id} className={`flex ${msg.sender === 'user' ? 'justify-end' : 'justify-start'}`}>
                  <div className={`max-w-[85%] p-3 rounded-xl border ${
                    msg.sender === 'user' 
                      ? 'bg-[rgb(var(--surface-rgb)/0.7)] border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-primary)]' 
                      : isError
                      ? 'bg-[rgb(var(--danger-rgb)/0.12)] border-[rgb(var(--danger-rgb)/0.35)] text-[rgb(var(--danger-rgb)/0.95)]'
                      : 'bg-[var(--bg-muted)] border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-primary)]'
                  }`}>
                    <div className="text-[9px] font-bold uppercase opacity-50 mb-1">{msg.sender}</div>
                    <div className={`text-xs leading-relaxed ${isError ? 'whitespace-pre-line' : ''}`}>{msg.content}</div>
                  </div>
                </div>
              );
            })}
            {orchestrator.status === TwinStatus.THINKING && (
              <div className="flex justify-start">
                <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] p-2 rounded-lg italic text-[10px] text-[var(--text-secondary)]">
                  {orchestrator.name} synthesizing mission parameters...
                </div>
              </div>
            )}
          </div>

          <div className="p-4 bg-[var(--bg-secondary)] border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
            <form onSubmit={handleSubmit} className="flex gap-2">
              <input 
                value={input}
                onChange={e => setInput(e.target.value)}
                placeholder="Global directives..."
                className="flex-1 bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg px-3 py-2 text-xs focus:ring-1 focus:ring-[rgb(var(--bg-steel-rgb)/0.4)]"
              />

              <button
                type="button"
                title={
                  !stt.state.isSupported
                    ? 'Voice input not supported in this browser'
                    : stt.state.isListening
                      ? 'Stop voice input'
                      : 'Start voice input'
                }
                disabled={!stt.state.isSupported || orchestrator.status === TwinStatus.THINKING}
                onClick={() => {
                  if (!stt.state.isSupported) return;
                  if (stt.state.isListening) {
                    stt.actions.stop();
                    return;
                  }
                  dictationBaseRef.current = input;
                  lastAppliedDictationRef.current = '';
                  stt.actions.start();
                }}
                className={
                  'px-3 py-2 rounded-lg text-xs font-bold transition-all border border-[rgb(var(--bg-steel-rgb)/0.3)] ' +
                  (!stt.state.isSupported || orchestrator.status === TwinStatus.THINKING
                    ? 'opacity-40 cursor-not-allowed bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--text-secondary)]'
                    : stt.state.isListening
                      ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                      : 'bg-[rgb(var(--surface-rgb)/0.7)] text-[var(--text-secondary)] hover:bg-[var(--bg-muted)] hover:text-[var(--text-on-accent)]')
                }
              >
                <span className="material-symbols-outlined text-[16px] align-middle">mic</span>
              </button>

              <button 
                type="submit"
                className="px-4 py-2 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] rounded-lg text-xs font-bold transition-all text-[var(--text-on-accent)]"
              >
                Execute
              </button>
            </form>

            {stt.state.error && (
              <div className="mt-2 text-[11px] text-[rgb(var(--danger-rgb)/0.85)] bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--danger-rgb)/0.35)] rounded-lg px-3 py-2">
                Voice input error: {stt.state.error}
              </div>
            )}
          </div>
            </div>

            {/* Crew List Sidebar */}
            <div className="w-80 shrink-0 flex flex-col">
              <CrewList twinId={orchestrator.id} />
            </div>
          </>
        ) : (
          /* Intelligence Tab - Consolidated Network & Phoenix */
          <div className="flex-1 flex flex-col min-h-0">
            {/* Intelligence View Toggle */}
            <div className="flex items-center justify-center gap-2 p-2 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
              <button
                onClick={() => setIntelligenceView('network')}
                className={`px-4 py-1.5 text-xs font-bold transition-all rounded-lg ${
                  intelligenceView === 'network'
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                    : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.6)] hover:text-[var(--text-secondary)] hover:bg-[rgb(var(--bg-muted-rgb)/0.5)]'
                }`}
              >
                <span className="material-symbols-outlined text-sm align-middle mr-1">hub</span>
                Network Topology
              </button>
              <button
                onClick={() => setIntelligenceView('memory')}
                className={`px-4 py-1.5 text-xs font-bold transition-all rounded-lg ${
                  intelligenceView === 'memory'
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                    : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[rgb(var(--text-secondary-rgb)/0.6)] hover:text-[var(--text-secondary)] hover:bg-[rgb(var(--bg-muted-rgb)/0.5)]'
                }`}
              >
                <span className="material-symbols-outlined text-sm align-middle mr-1">memory</span>
                Memory Flow
              </button>
            </div>
            
            {/* Intelligence Content */}
            <div className="flex-1 flex flex-col min-h-0 p-4">
              {intelligenceView === 'network' ? (
                <TrustNetworkMap className="flex-1" />
              ) : (
                <PhoenixMemoryFlow />
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default OrchestratorHub;
