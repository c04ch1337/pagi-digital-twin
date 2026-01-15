import React, { useState, useRef, useEffect } from 'react';
import { Message, Twin, TwinStatus } from '../types';
import { AVAILABLE_TOOLS } from '../constants';
import { commitToMemory } from '../services/memory';
import { useSpeechToText } from '../hooks/useSpeechToText';
import { getUserName } from '../utils/userName';
import { useDomainAttribution } from '../context/DomainAttributionContext';

interface ChatAreaProps {
  messages: Message[];
  activeTwin: Twin;
  onSendMessage: (text: string) => void;
  onRunTool: (toolId: string) => void;
  onOpenSettings?: () => void;
}

const ChatArea: React.FC<ChatAreaProps> = ({ messages, activeTwin, onSendMessage, onRunTool, onOpenSettings }) => {
  const { getAttributionForMessage, getDominantDomain } = useDomainAttribution();
  const [input, setInput] = useState('');
  const [isToolMenuOpen, setIsToolMenuOpen] = useState(false);
  const [saveStatus, setSaveStatus] = useState<Record<string, boolean>>({});
  const scrollRef = useRef<HTMLDivElement>(null);
  const toolMenuRef = useRef<HTMLDivElement>(null);

  const stt = useSpeechToText({ lang: 'en-US', continuous: false, interimResults: true });
  const dictationBaseRef = useRef<string>('');
  const lastAppliedDictationRef = useRef<string>('');

  useEffect(() => {
    if (!stt.state.isListening) return;
    const base = dictationBaseRef.current;
    const parts = [base, stt.state.finalText, stt.state.interimText].filter(Boolean);
    const next = parts.join(' ').replace(/\s+/g, ' ').trimStart();
    if (lastAppliedDictationRef.current === next) return;
    lastAppliedDictationRef.current = next;
    setInput(next);
  }, [stt.state.finalText, stt.state.interimText, stt.state.isListening]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (toolMenuRef.current && !toolMenuRef.current.contains(event.target as Node)) {
        setIsToolMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (input.trim()) {
      onSendMessage(input.trim());
      setInput('');
    }
  };

  const handleSaveToMemory = (msg: Message) => {
    if (saveStatus[msg.id]) return;
    
    commitToMemory(activeTwin.settings.memoryNamespace, msg.content);
    setSaveStatus(prev => ({ ...prev, [msg.id]: true }));
    
    // Auto-clear success state after 2 seconds
    setTimeout(() => {
      setSaveStatus(prev => ({ ...prev, [msg.id]: false }));
    }, 2000);
  };

  const authorizedTools = AVAILABLE_TOOLS.filter(tool => 
    activeTwin.settings.toolAccess.includes(tool.id)
  );

  // Helper function to get user's first name
  const getUserFirstName = (): string => {
    const fullName = getUserName();
    if (fullName === 'FG_User') {
      return 'FG_User';
    }
    // Extract first name (everything before the first space)
    const firstName = fullName.split(' ')[0];
    return firstName || fullName;
  };

  // Get display name for message sender
  const getSenderDisplayName = (sender: 'user' | 'assistant'): string => {
    if (sender === 'user') {
      return getUserFirstName();
    }
    return activeTwin.name;
  };

  return (
    <div className="chat-area flex-1 flex flex-col bg-[var(--bg-primary)] relative">
      <div 
        ref={scrollRef}
        className="relative z-10 flex-1 overflow-y-auto p-4 md:p-8 space-y-6"
      >
        {messages.length === 0 && (
          <div className="h-full flex flex-col items-center justify-center text-center max-w-md mx-auto">
             <div className="w-16 h-16 rounded-3xl bg-[rgb(var(--surface-rgb)/0.5)] flex items-center justify-center mb-6 border border-[rgb(var(--bg-steel-rgb)/0.3)]">
               <img src={activeTwin.avatar} className="w-12 h-12 rounded-2xl grayscale" />
             </div>
             <h2 className="text-xl font-bold text-[var(--text-primary)] mb-2">Initialize {activeTwin.name}</h2>
             <p className="text-[var(--text-secondary)] text-sm">
                This agent is active as a{' '}
                {onOpenSettings ? (
                  <button
                    onClick={onOpenSettings}
                    className="text-[var(--bg-steel)] font-medium hover:text-[rgb(var(--bg-steel-rgb)/0.85)] hover:underline transition-colors cursor-pointer"
                    title="Click to configure agent settings"
                  >
                    {activeTwin.role}
                  </button>
                ) : (
                  <span className="text-[var(--bg-steel)] font-medium">{activeTwin.role}</span>
                )}. 
                State the security objective to begin tactical orchestration.
             </p>
           </div>
        )}

        {messages.map((msg) => (
          <div 
            key={msg.id} 
            className={`flex gap-4 ${msg.sender === 'user' ? 'justify-end' : 'justify-start'}`}
          >
            {msg.sender === 'assistant' && (
              <div className="w-8 h-8 rounded-lg overflow-hidden shrink-0 border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.3)]">
                <img src={activeTwin.avatar} className="w-full h-full object-cover" />
              </div>
            )}
            
            <div className={`max-w-[85%] md:max-w-[70%] space-y-2 group relative`}>
              <div className={`flex items-center gap-2 text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-wider mb-1 px-1 ${
                msg.sender === 'user' ? 'justify-end' : 'justify-start'
              }`}>
                <span>{getSenderDisplayName(msg.sender)}</span>
                {msg.sender === 'assistant' && (() => {
                  const attribution = getAttributionForMessage(msg.id);
                  const dominantDomain = getDominantDomain(attribution);
                  if (!dominantDomain) return null;
                  
                  const domainLabels = { M: 'Mind', B: 'Body', H: 'Heart', S: 'Soul' };
                  const domainColors = {
                    M: 'text-[var(--bg-steel)] bg-[rgb(var(--bg-steel-rgb)/0.15)] border-[rgb(var(--bg-steel-rgb)/0.3)]',
                    B: 'text-[var(--bg-steel)] bg-[rgb(var(--bg-steel-rgb)/0.15)] border-[rgb(var(--bg-steel-rgb)/0.3)]',
                    H: 'text-[rgb(var(--warning-rgb))] bg-[rgb(var(--warning-rgb)/0.15)] border-[rgb(var(--warning-rgb)/0.3)]',
                    S: 'text-[rgb(var(--danger-rgb))] bg-[rgb(var(--danger-rgb)/0.15)] border-[rgb(var(--danger-rgb)/0.3)]',
                  };
                  
                  return (
                    <span 
                      className={`px-1.5 py-0.5 rounded text-[8px] font-bold border ${domainColors[dominantDomain]}`}
                      title={`Domain: ${domainLabels[dominantDomain]} (${attribution ? Math.round(attribution[dominantDomain === 'M' ? 'mind' : dominantDomain === 'B' ? 'body' : dominantDomain === 'H' ? 'heart' : 'soul']) : 0}%)`}
                    >
                      {dominantDomain}
                    </span>
                  );
                })()}
              </div>
              <div className={`p-4 rounded-2xl text-sm leading-relaxed shadow-sm ${
                msg.sender === 'user' 
                  ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded-tr-none' 
                  : (msg.content.includes('[ERROR]') || msg.content.includes('Connection Error') || msg.content.includes('Network Error'))
                  ? 'bg-[rgb(var(--danger-rgb)/0.12)] text-[rgb(var(--danger-rgb)/0.95)] border border-[rgb(var(--danger-rgb)/0.35)] rounded-tl-none'
                  : 'bg-[rgb(var(--surface-rgb)/0.7)] text-[var(--text-primary)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-tl-none'
              }`}>
                <div className={msg.content.includes('Connection Error') || msg.content.includes('Network Error') ? 'whitespace-pre-line' : ''}>
                  {msg.content}
                </div>
                
                {/* Save to Memory Button */}
                {msg.sender === 'assistant' && (
                  <button 
                    onClick={() => handleSaveToMemory(msg)}
                    className={`absolute -right-10 top-2 p-1.5 rounded-lg border transition-all opacity-0 group-hover:opacity-100 ${
                      saveStatus[msg.id] ? 'bg-[rgb(var(--surface-rgb)/0.7)] border-[var(--bg-muted)] text-[var(--bg-steel)]' : 'bg-[rgb(var(--surface-rgb)/0.6)] border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-secondary)] hover:text-[var(--bg-steel)]'
                    }`}
                    title="Commit to Vector Vault"
                  >
                    <span className="material-symbols-outlined text-[16px]">
                      {saveStatus[msg.id] ? 'check_circle' : 'database'}
                    </span>
                  </button>
                )}
              </div>
              <div className={`text-[10px] text-[var(--text-secondary)] px-1 ${msg.sender === 'user' ? 'text-right' : 'text-left'}`}>
                {msg.timestamp.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
              </div>
            </div>

            {msg.sender === 'user' && (
               <div className="w-8 h-8 rounded-lg bg-[rgb(var(--surface-rgb)/0.5)] flex items-center justify-center shrink-0 border border-[rgb(var(--bg-steel-rgb)/0.3)]">
                 <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
               </div>
            )}
          </div>
        ))}
        
        {activeTwin.status === TwinStatus.THINKING && (
          <div className="flex gap-4 justify-start animate-in fade-in slide-in-from-bottom-2 duration-300">
             <div className="w-8 h-8 rounded-lg overflow-hidden shrink-0 border border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center justify-center bg-[rgb(var(--surface-rgb)/0.6)]">
                <div className="w-4 h-4 rounded-full border-2 border-[var(--bg-muted)] border-t-[var(--bg-steel)] animate-spin" />
              </div>
             <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] p-3 rounded-2xl rounded-tl-none italic text-[var(--text-secondary)] text-xs">
                Scanning neural telemetry for threats...
              </div>
          </div>
        )}
      </div>

      <div className="relative z-10 p-4 md:p-6 bg-gradient-to-t from-[var(--bg-secondary)] via-[var(--bg-secondary)] to-transparent">
        <div className="max-w-4xl mx-auto relative">
          <form 
            onSubmit={handleSubmit}
            className="bg-[rgb(var(--surface-rgb)/0.7)] backdrop-blur-md border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-2xl p-2 shadow-2xl flex items-center gap-2 group focus-within:ring-2 focus-within:ring-[rgb(var(--bg-steel-rgb)/0.3)] transition-all"
          >
            <button 
              type="button" 
              onClick={() => setIsToolMenuOpen(!isToolMenuOpen)}
              className={`p-2 rounded-xl transition-all flex items-center gap-2 ${isToolMenuOpen ? 'bg-[var(--bg-muted)] text-[var(--text-primary)]' : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'}`}
            >
              <span className="material-symbols-outlined">construction</span>
              <span className="text-[10px] font-bold uppercase tracking-widest hidden sm:inline">Run Tool</span>
            </button>
            
            <div className="h-6 w-px bg-[rgb(var(--bg-steel-rgb)/0.3)] mx-1" />

            <input 
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder={`Instruct ${activeTwin.name}...`}
              className="flex-1 bg-transparent border-none focus:ring-0 text-sm text-[var(--text-primary)] placeholder-[var(--text-secondary)] py-2"
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
              disabled={!stt.state.isSupported || activeTwin.status === TwinStatus.THINKING}
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
                'p-2 rounded-xl transition-all flex items-center justify-center ' +
                (!stt.state.isSupported || activeTwin.status === TwinStatus.THINKING
                  ? 'opacity-40 cursor-not-allowed'
                  : stt.state.isListening
                    ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
                    : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]')
              }
            >
              <span className="material-symbols-outlined text-[18px]">mic</span>
            </button>
            
            <button 
              type="submit"
              disabled={!input.trim() || activeTwin.status === TwinStatus.THINKING}
              className="p-2 bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded-xl hover:bg-[var(--bg-muted)] disabled:opacity-50 transition-all flex items-center gap-2 pr-3"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>
              <span className="text-[10px] font-bold uppercase tracking-widest hidden sm:inline">Send</span>
            </button>
          </form>

          {stt.state.error && (
            <div className="mt-2 text-[11px] text-[rgb(var(--danger-rgb)/0.85)] bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--danger-rgb)/0.35)] rounded-lg px-3 py-2">
              Voice input error: {stt.state.error}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default ChatArea;
