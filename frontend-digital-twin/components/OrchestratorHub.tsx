
import React, { useState, useRef, useEffect } from 'react';
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
    <div className="flex-1 flex flex-col bg-[#9EC9D9] overflow-hidden relative">
      {/* Background Tactical Grid */}
      <div className="absolute inset-0 opacity-[0.03] pointer-events-none" 
           style={{ backgroundImage: 'linear-gradient(#5381A5 1px, transparent 1px), linear-gradient(90deg, #5381A5 1px, transparent 1px)', backgroundSize: '40px 40px' }} />

      <div className="flex-1 flex flex-col md:flex-row min-h-0">
        {/* Unified Command Chat */}
        <div className="flex-1 flex flex-col">
          <div className="p-4 border-b border-[#5381A5]/30 flex items-center justify-between bg-[#90C3EA]">
             <div className="flex items-center gap-2">
                <span className="material-symbols-outlined text-[#5381A5]">terminal</span>
                <span className="text-[10px] font-bold uppercase tracking-widest text-[#163247]">Powered by Phoenix AGI (PAGI OS v0.1)</span>
             </div>
             <div className="text-[9px] text-[#163247] font-mono">TRANSPORT: Unencrypted (Dev Mode)</div>
          </div>
          
          <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-4">
            {messages.filter(m => m.twinId === orchestrator.id || m.sender === 'user').map(msg => {
              const isError = msg.content.includes('[ERROR]') || msg.content.includes('Connection Error') || msg.content.includes('Network Error');
              return (
                <div key={msg.id} className={`flex ${msg.sender === 'user' ? 'justify-end' : 'justify-start'}`}>
                  <div className={`max-w-[85%] p-3 rounded-xl border ${
                    msg.sender === 'user' 
                      ? 'bg-white/70 border-[#5381A5]/30 text-[#0b1b2b]' 
                      : isError
                      ? 'bg-red-50 border-red-300 text-red-900'
                      : 'bg-[#78A2C2] border-[#5381A5]/30 text-[#0b1b2b]'
                  }`}>
                    <div className="text-[9px] font-bold uppercase opacity-50 mb-1">{msg.sender}</div>
                    <div className={`text-xs leading-relaxed ${isError ? 'whitespace-pre-line' : ''}`}>{msg.content}</div>
                  </div>
                </div>
              );
            })}
            {orchestrator.status === TwinStatus.THINKING && (
              <div className="flex justify-start">
                <div className="bg-white/60 border border-[#5381A5]/30 p-2 rounded-lg italic text-[10px] text-[#163247]">
                  The Blue Flame synthesizing mission parameters...
                </div>
              </div>
            )}
          </div>

          <div className="p-4 bg-[#90C3EA] border-t border-[#5381A5]/30">
            <form onSubmit={handleSubmit} className="flex gap-2">
              <input 
                value={input}
                onChange={e => setInput(e.target.value)}
                placeholder="Global directives..."
                className="flex-1 bg-white/70 border border-[#5381A5]/30 rounded-lg px-3 py-2 text-xs focus:ring-1 focus:ring-[#5381A5]/40"
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
                  'px-3 py-2 rounded-lg text-xs font-bold transition-all border border-[#5381A5]/30 ' +
                  (!stt.state.isSupported || orchestrator.status === TwinStatus.THINKING
                    ? 'opacity-40 cursor-not-allowed bg-white/40 text-[#163247]'
                    : stt.state.isListening
                      ? 'bg-[#5381A5] text-white'
                      : 'bg-white/70 text-[#163247] hover:bg-[#78A2C2] hover:text-white')
                }
              >
                <span className="material-symbols-outlined text-[16px] align-middle">mic</span>
              </button>

              <button 
                type="submit"
                className="px-4 py-2 bg-[#5381A5] hover:bg-[#78A2C2] rounded-lg text-xs font-bold transition-all text-white"
              >
                Execute
              </button>
            </form>

            {stt.state.error && (
              <div className="mt-2 text-[11px] text-red-700 bg-white/60 border border-red-300/60 rounded-lg px-3 py-2">
                Voice input error: {stt.state.error}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default OrchestratorHub;
