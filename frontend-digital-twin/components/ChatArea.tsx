import React, { useState, useRef, useEffect } from 'react';
import { Message, Twin, TwinStatus } from '../types';
import { AVAILABLE_TOOLS } from '../constants';
import { commitToMemory } from '../services/memory';

interface ChatAreaProps {
  messages: Message[];
  activeTwin: Twin;
  onSendMessage: (text: string) => void;
  onRunTool: (toolId: string) => void;
}

const ChatArea: React.FC<ChatAreaProps> = ({ messages, activeTwin, onSendMessage, onRunTool }) => {
  const [input, setInput] = useState('');
  const [isToolMenuOpen, setIsToolMenuOpen] = useState(false);
  const [saveStatus, setSaveStatus] = useState<Record<string, boolean>>({});
  const scrollRef = useRef<HTMLDivElement>(null);
  const toolMenuRef = useRef<HTMLDivElement>(null);

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

  return (
    <div className="flex-1 flex flex-col bg-[#9EC9D9] relative">
      <div 
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 md:p-8 space-y-6"
      >
        {messages.length === 0 && (
          <div className="h-full flex flex-col items-center justify-center text-center max-w-md mx-auto">
             <div className="w-16 h-16 rounded-3xl bg-white/50 flex items-center justify-center mb-6 border border-[#5381A5]/30">
               <img src={activeTwin.avatar} className="w-12 h-12 rounded-2xl grayscale" />
             </div>
             <h2 className="text-xl font-bold text-[#0b1b2b] mb-2">Initialize {activeTwin.name}</h2>
             <p className="text-[#163247] text-sm">
                This agent is active as a <span className="text-[#5381A5] font-medium">{activeTwin.role}</span>. 
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
              <div className="w-8 h-8 rounded-lg overflow-hidden shrink-0 border border-[#5381A5]/30 bg-white/30">
                <img src={activeTwin.avatar} className="w-full h-full object-cover" />
              </div>
            )}
            
            <div className={`max-w-[85%] md:max-w-[70%] space-y-2 group relative`}>
              <div className={`p-4 rounded-2xl text-sm leading-relaxed shadow-sm ${
                msg.sender === 'user' 
                  ? 'bg-[#5381A5] text-white rounded-tr-none' 
                  : 'bg-white/70 text-[#0b1b2b] border border-[#5381A5]/30 rounded-tl-none'
              }`}>
                {msg.content}
                
                {/* Save to Memory Button */}
                {msg.sender === 'assistant' && (
                  <button 
                    onClick={() => handleSaveToMemory(msg)}
                    className={`absolute -right-10 top-2 p-1.5 rounded-lg border transition-all opacity-0 group-hover:opacity-100 ${
                      saveStatus[msg.id] ? 'bg-white/70 border-[#78A2C2] text-[#5381A5]' : 'bg-white/60 border-[#5381A5]/30 text-[#163247] hover:text-[#5381A5]'
                    }`}
                    title="Commit to Vector Vault"
                  >
                    <span className="material-symbols-outlined text-[16px]">
                      {saveStatus[msg.id] ? 'check_circle' : 'database'}
                    </span>
                  </button>
                )}
              </div>
              <div className={`text-[10px] text-[#163247] px-1 ${msg.sender === 'user' ? 'text-right' : 'text-left'}`}>
                {msg.timestamp.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
              </div>
            </div>

            {msg.sender === 'user' && (
               <div className="w-8 h-8 rounded-lg bg-white/50 flex items-center justify-center shrink-0 border border-[#5381A5]/30">
                 <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
               </div>
            )}
          </div>
        ))}
        
        {activeTwin.status === TwinStatus.THINKING && (
          <div className="flex gap-4 justify-start animate-in fade-in slide-in-from-bottom-2 duration-300">
             <div className="w-8 h-8 rounded-lg overflow-hidden shrink-0 border border-[#5381A5]/30 flex items-center justify-center bg-white/60">
                <div className="w-4 h-4 rounded-full border-2 border-[#78A2C2] border-t-[#5381A5] animate-spin" />
              </div>
             <div className="bg-white/60 border border-[#5381A5]/30 p-3 rounded-2xl rounded-tl-none italic text-[#163247] text-xs">
                Scanning neural telemetry for threats...
              </div>
          </div>
        )}
      </div>

      <div className="p-4 md:p-6 bg-gradient-to-t from-[#90C3EA] via-[#90C3EA] to-transparent">
        <div className="max-w-4xl mx-auto relative">
          <form 
            onSubmit={handleSubmit}
            className="bg-white/70 backdrop-blur-md border border-[#5381A5]/30 rounded-2xl p-2 shadow-2xl flex items-center gap-2 group focus-within:ring-2 focus-within:ring-[#5381A5]/30 transition-all"
          >
            <button 
              type="button" 
              onClick={() => setIsToolMenuOpen(!isToolMenuOpen)}
              className={`p-2 rounded-xl transition-all flex items-center gap-2 ${isToolMenuOpen ? 'bg-[#78A2C2] text-[#0b1b2b]' : 'text-[#163247] hover:text-[#0b1b2b]'}`}
            >
              <span className="material-symbols-outlined">construction</span>
              <span className="text-[10px] font-bold uppercase tracking-widest hidden sm:inline">Run Tool</span>
            </button>
            
            <div className="h-6 w-px bg-[#5381A5]/30 mx-1" />

            <input 
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder={`Instruct ${activeTwin.name}...`}
              className="flex-1 bg-transparent border-none focus:ring-0 text-sm text-[#0b1b2b] placeholder-[#163247] py-2"
            />
            
            <button 
              type="submit"
              disabled={!input.trim() || activeTwin.status === TwinStatus.THINKING}
              className="p-2 bg-[#5381A5] text-white rounded-xl hover:bg-[#78A2C2] disabled:opacity-50 transition-all flex items-center gap-2 pr-3"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>
              <span className="text-[10px] font-bold uppercase tracking-widest hidden sm:inline">Send</span>
            </button>
          </form>
        </div>
      </div>
    </div>
  );
};

export default ChatArea;
