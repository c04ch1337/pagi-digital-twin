
import React, { useState, useRef, useEffect } from 'react';
import { Message, Twin, TwinStatus } from '../types';
import { generateTacticalImage, generateDeepVideo } from '../services/gemini';

interface OrchestratorHubProps {
  orchestrator: Twin;
  messages: Message[];
  onSendMessage: (text: string) => void;
}

const OrchestratorHub: React.FC<OrchestratorHubProps> = ({ orchestrator, messages, onSendMessage }) => {
  const [input, setInput] = useState('');
  const [isGenerating, setIsGenerating] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (input.trim()) {
      onSendMessage(input.trim());
      setInput('');
    }
  };

  const handleGenTask = async (type: 'image' | 'video' | 'code') => {
    if (!input.trim()) return;
    
    // Check for AI code generation policy if type is code
    if (type === 'code' && !orchestrator.settings.aiCodeGenerationEnabled) {
      onSendMessage("[POLICY DENIED] AI code generation is disabled in the Defensive Policies settings for this node.");
      return;
    }

    setIsGenerating(true);
    const userPrompt = input.trim();
    setInput('');
    
    // Add user message for record
    onSendMessage(`[REQUEST: ${type.toUpperCase()}] ${userPrompt}`);

    if (type === 'image') {
      const imgUrl = await generateTacticalImage(userPrompt);
      if (imgUrl) {
         // In a full implementation, we'd add this to messages state properly
         console.log("Image Gen Success:", imgUrl);
      }
    } else if (type === 'video') {
      await generateDeepVideo(userPrompt);
    }
    
    setIsGenerating(false);
  };

  const isCodeDisabled = !orchestrator.settings.aiCodeGenerationEnabled;

  return (
    <div className="flex-1 flex flex-col bg-[#050507] overflow-hidden relative">
      {/* Background Tactical Grid */}
      <div className="absolute inset-0 opacity-[0.03] pointer-events-none" 
           style={{ backgroundImage: 'linear-gradient(#4f46e5 1px, transparent 1px), linear-gradient(90deg, #4f46e5 1px, transparent 1px)', backgroundSize: '40px 40px' }} />

      <div className="flex-1 flex flex-col md:flex-row min-h-0">
        {/* Unified Command Chat */}
        <div className="flex-1 flex flex-col border-r border-zinc-800/50">
          <div className="p-4 border-b border-zinc-800/50 flex items-center justify-between bg-zinc-950/30">
             <div className="flex items-center gap-2">
                <span className="material-symbols-outlined text-indigo-500">terminal</span>
                <span className="text-[10px] font-bold uppercase tracking-widest text-zinc-400">Direct Command Stream</span>
             </div>
             <div className="text-[9px] text-zinc-600 font-mono">ENCRYPTION: AES-256-QUANTUM</div>
          </div>
          
          <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-4">
            {messages.filter(m => m.twinId === orchestrator.id || m.sender === 'user').map(msg => (
              <div key={msg.id} className={`flex ${msg.sender === 'user' ? 'justify-end' : 'justify-start'}`}>
                <div className={`max-w-[85%] p-3 rounded-xl border ${
                  msg.sender === 'user' 
                    ? 'bg-zinc-900 border-zinc-800 text-zinc-200' 
                    : 'bg-indigo-600/10 border-indigo-600/30 text-indigo-100'
                }`}>
                  <div className="text-[9px] font-bold uppercase opacity-50 mb-1">{msg.sender}</div>
                  <div className="text-xs leading-relaxed">{msg.content}</div>
                </div>
              </div>
            ))}
            {orchestrator.status === TwinStatus.THINKING && (
              <div className="flex justify-start">
                <div className="bg-zinc-900/50 border border-zinc-800 p-2 rounded-lg italic text-[10px] text-zinc-500">
                  Orchestrator synthesizing mission parameters...
                </div>
              </div>
            )}
          </div>

          <div className="p-4 bg-zinc-950/50 border-t border-zinc-800/50">
            <form onSubmit={handleSubmit} className="flex gap-2">
              <input 
                value={input}
                onChange={e => setInput(e.target.value)}
                placeholder="Global directives or AI task prompt..."
                className="flex-1 bg-zinc-900/50 border border-zinc-800 rounded-lg px-3 py-2 text-xs focus:ring-1 focus:ring-indigo-500/50"
              />
              <button 
                type="submit"
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 rounded-lg text-xs font-bold transition-all"
              >
                Execute
              </button>
            </form>
          </div>
        </div>

        {/* Task Matrix & Generative Controls */}
        <div className="w-full md:w-80 bg-zinc-950/20 p-4 space-y-4 overflow-y-auto">
          <div className="space-y-1">
            <h3 className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest px-1">Generative Tasks</h3>
            <p className="text-[9px] text-zinc-600 px-1 italic mb-4">Requires active instruction payload in chat</p>
          </div>

          <div className="grid grid-cols-1 gap-3">
             <button 
                onClick={() => handleGenTask('image')}
                disabled={isGenerating || !input.trim()}
                className="group relative bg-zinc-900 border border-zinc-800 p-4 rounded-2xl text-left hover:border-indigo-500/50 transition-all overflow-hidden disabled:opacity-50"
             >
                <div className="absolute top-0 right-0 p-2 opacity-10 group-hover:opacity-30 transition-opacity">
                  <span className="material-symbols-outlined text-4xl">image</span>
                </div>
                <div className="relative z-10">
                  <div className="text-xs font-bold text-zinc-200">Generate Visual Evidence</div>
                  <div className="text-[9px] text-zinc-500 mt-1 leading-tight">Gemini 2.5 Flash • 1K Tactical Visuals</div>
                </div>
             </button>

             <button 
                onClick={() => handleGenTask('video')}
                disabled={isGenerating || !input.trim()}
                className="group relative bg-zinc-900 border border-zinc-800 p-4 rounded-2xl text-left hover:border-indigo-500/50 transition-all overflow-hidden disabled:opacity-50"
             >
                <div className="absolute top-0 right-0 p-2 opacity-10 group-hover:opacity-30 transition-opacity">
                  <span className="material-symbols-outlined text-4xl">movie</span>
                </div>
                <div className="relative z-10">
                  <div className="text-xs font-bold text-zinc-200">Reconstruct Scenario</div>
                  <div className="text-[9px] text-zinc-500 mt-1 leading-tight">Veo 3.1 • Deep Video Synthesis</div>
                </div>
             </button>

             <button 
                onClick={() => handleGenTask('code')}
                disabled={isGenerating || !input.trim() || isCodeDisabled}
                className={`group relative bg-zinc-900 border p-4 rounded-2xl text-left transition-all overflow-hidden disabled:opacity-50 ${isCodeDisabled ? 'border-zinc-800 opacity-40 cursor-not-allowed' : 'border-zinc-800 hover:border-indigo-500/50'}`}
             >
                <div className="absolute top-0 right-0 p-2 opacity-10 group-hover:opacity-30 transition-opacity">
                  <span className="material-symbols-outlined text-4xl">{isCodeDisabled ? 'lock' : 'code'}</span>
                </div>
                <div className="relative z-10">
                  <div className={`text-xs font-bold ${isCodeDisabled ? 'text-zinc-500' : 'text-zinc-200'}`}>
                    Synthesize Patch {isCodeDisabled && '(Locked)'}
                  </div>
                  <div className="text-[9px] text-zinc-500 mt-1 leading-tight">
                    {isCodeDisabled ? 'Policy: AI Code Generation Disabled' : 'Gemini 3 Pro • Advanced Logic'}
                  </div>
                </div>
             </button>
          </div>

          <div className="pt-6 border-t border-zinc-800/50">
             <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-3">Global Mission Status</div>
             <div className="bg-zinc-900/40 p-3 rounded-xl border border-zinc-800/30 space-y-4">
                <div className="space-y-2">
                   <div className="flex justify-between text-[9px] text-zinc-400">
                      <span>Neural Sync</span>
                      <span className="text-indigo-400">98%</span>
                   </div>
                   <div className="h-1 bg-zinc-800 rounded-full overflow-hidden">
                      <div className="h-full bg-indigo-500 w-[98%]" />
                   </div>
                </div>
                <div className="space-y-2">
                   <div className="flex justify-between text-[9px] text-zinc-400">
                      <span>Threat Suppression</span>
                      <span className="text-emerald-400">72%</span>
                   </div>
                   <div className="h-1 bg-zinc-800 rounded-full overflow-hidden">
                      <div className="h-full bg-emerald-500 w-[72%]" />
                   </div>
                </div>
             </div>
          </div>
        </div>
      </div>

      {isGenerating && (
        <div className="absolute inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center">
          <div className="flex flex-col items-center gap-4">
             <div className="w-12 h-12 border-4 border-indigo-500 border-t-transparent rounded-full animate-spin" />
             <div className="text-xs font-bold text-indigo-400 animate-pulse tracking-widest">SYNTHESIZING TACTICAL ASSET...</div>
          </div>
        </div>
      )}
    </div>
  );
};

export default OrchestratorHub;
