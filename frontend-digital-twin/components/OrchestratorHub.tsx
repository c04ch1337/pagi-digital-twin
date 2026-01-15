
import React, { useEffect, useRef, useState } from 'react';
import { Message, Twin, TwinStatus } from '../types';
import { useSpeechToText } from '../hooks/useSpeechToText';

interface OrchestratorHubProps {
  orchestrator: Twin;
  messages: Message[];
  onSendMessage: (text: string) => void;
}

const OrchestratorHub: React.FC<OrchestratorHubProps> = ({ orchestrator, messages, onSendMessage }) => {
  const [input, setInput] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);

  const stt = useSpeechToText({ lang: 'en-US', continuous: false, interimResults: true });
  const dictationBaseRef = useRef<string>('');
  const lastAppliedDictationRef = useRef<string>('');

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

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

      {/* Unified Command Chat (full-height, no tabs) */}
      <div className="flex-1 flex flex-col min-h-0">
        <div className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center justify-between bg-[var(--bg-secondary)]">
          <div className="flex items-center gap-2">
            <span className="material-symbols-outlined text-[var(--bg-steel)]">terminal</span>
            <span className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)]">
              Powered by Phoenix AGI (PAGI OS v0.1)
            </span>
          </div>
          <div className="text-[9px] text-[var(--text-secondary)] font-mono">TRANSPORT: Unencrypted (Dev Mode)</div>
        </div>

        <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-4">
          {messages
            .filter((m) => m.twinId === orchestrator.id || m.sender === 'user')
            .map((msg) => {
              const isError =
                msg.content.includes('[ERROR]') ||
                msg.content.includes('Connection Error') ||
                msg.content.includes('Network Error');
              return (
                <div key={msg.id} className={`flex ${msg.sender === 'user' ? 'justify-end' : 'justify-start'}`}>
                  <div
                    className={`max-w-[85%] p-3 rounded-xl border ${
                      msg.sender === 'user'
                        ? 'bg-[rgb(var(--surface-rgb)/0.7)] border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-primary)]'
                        : isError
                          ? 'bg-[rgb(var(--danger-rgb)/0.12)] border-[rgb(var(--danger-rgb)/0.35)] text-[rgb(var(--danger-rgb)/0.95)]'
                          : 'bg-[var(--bg-muted)] border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-primary)]'
                    }`}
                  >
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
              onChange={(e) => setInput(e.target.value)}
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
    </div>
  );
};

export default OrchestratorHub;
