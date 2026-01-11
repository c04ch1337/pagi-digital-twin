
import React, { useState } from 'react';
import { Twin, TwinStatus } from '../types';

interface CreateTwinModalProps {
  onSave: (twin: Twin) => void;
  onClose: () => void;
}

const CreateTwinModal: React.FC<CreateTwinModalProps> = ({ onSave, onClose }) => {
  const [formData, setFormData] = useState({
    name: '',
    role: '',
    description: '',
    systemPrompt: '',
    avatar: `https://picsum.photos/seed/${Math.random()}/200`
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.name || !formData.role) return;

    const newTwin: Twin = {
      id: `twin-${Date.now()}`,
      name: formData.name,
      role: formData.role,
      description: formData.description,
      avatar: formData.avatar,
      status: TwinStatus.IDLE,
      systemPrompt: formData.systemPrompt || `# TACTICAL DIRECTIVE\nYou are ${formData.name}, a ${formData.role}.`,
      capabilities: [],
      isTacticalNode: true,
      settings: {
        safeMode: true,
        toolAccess: ['vector_query'],
        maxMemory: 4,
        tokenLimit: 64,
        memoryNamespace: 'default',
        aiCodeGenerationEnabled: false,
        llmProvider: 'gemini',
        // Fix: Added missing temperature and topP properties required by TwinSettings interface
        temperature: 0.7,
        topP: 0.95
      }
    };

    onSave(newTwin);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-zinc-950/80 backdrop-blur-sm animate-in fade-in duration-200">
      <div className="w-full max-w-2xl bg-[#1a2232] border border-[#344465] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        <div className="p-6 border-b border-[#344465] flex items-center justify-between bg-[#111621]">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-[#195de6]">add_to_drive</span>
            <h2 className="text-xl font-bold text-white font-display">Register New Tactical Agent</h2>
          </div>
          <button 
            onClick={onClose}
            className="text-zinc-500 hover:text-white transition-colors"
          >
            <span className="material-symbols-outlined">close</span>
          </button>
        </div>

        <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto p-6 space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <label className="flex flex-col gap-2">
              <span className="text-[#93a5c8] text-xs font-bold uppercase tracking-widest">Codename</span>
              <input 
                required
                className="bg-[#111621] border border-[#344465] rounded-lg h-11 px-4 text-white focus:border-[#195de6] outline-none transition-all text-sm"
                placeholder="e.g. Shadow Vector"
                value={formData.name}
                onChange={e => setFormData({ ...formData, name: e.target.value })}
              />
            </label>
            <label className="flex flex-col gap-2">
              <span className="text-[#93a5c8] text-xs font-bold uppercase tracking-widest">Tactical Role</span>
              <input 
                required
                className="bg-[#111621] border border-[#344465] rounded-lg h-11 px-4 text-white focus:border-[#195de6] outline-none transition-all text-sm"
                placeholder="e.g. Signal Interceptor"
                value={formData.role}
                onChange={e => setFormData({ ...formData, role: e.target.value })}
              />
            </label>
            <label className="flex flex-col gap-2 md:col-span-2">
              <span className="text-[#93a5c8] text-xs font-bold uppercase tracking-widest">Mission Summary</span>
              <input 
                className="bg-[#111621] border border-[#344465] rounded-lg h-11 px-4 text-white focus:border-[#195de6] outline-none transition-all text-sm"
                placeholder="Brief description of the node's function"
                value={formData.description}
                onChange={e => setFormData({ ...formData, description: e.target.value })}
              />
            </label>
          </div>

          <label className="flex flex-col gap-2">
            <span className="text-[#93a5c8] text-xs font-bold uppercase tracking-widest">Directive Logic (System Prompt)</span>
            <textarea 
              rows={6}
              className="bg-[#111621] border border-[#344465] rounded-lg p-4 text-white font-mono text-sm focus:border-[#195de6] outline-none transition-all resize-none"
              placeholder="# OPERATIONAL MANDATE\nYou are a high-performance agent designed for..."
              value={formData.systemPrompt}
              onChange={e => setFormData({ ...formData, systemPrompt: e.target.value })}
            />
          </label>

          <div className="p-4 bg-indigo-500/5 border border-indigo-500/20 rounded-xl">
            <div className="flex items-start gap-3">
              <span className="material-symbols-outlined text-indigo-400 text-sm mt-0.5">info</span>
              <p className="text-[11px] text-[#93a5c8] leading-relaxed">
                By default, new agents are initialized in <strong>Safe Mode</strong> with restricted tool access. 
                You can adjust defensive policies in the agent configuration view after registration.
              </p>
            </div>
          </div>
        </form>

        <div className="p-6 border-t border-[#344465] bg-[#111621] flex justify-end gap-3">
          <button 
            type="button"
            onClick={onClose}
            className="px-6 py-2 rounded-lg text-sm font-bold text-[#93a5c8] hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button 
            type="submit"
            onClick={handleSubmit}
            className="bg-[#195de6] hover:bg-[#195de6]/90 text-white px-8 py-2 rounded-lg text-sm font-bold shadow-lg shadow-[#195de6]/20 transition-all flex items-center gap-2"
          >
            <span className="material-symbols-outlined text-sm">rocket_launch</span>
            Initialize Agent
          </button>
        </div>
      </div>
    </div>
  );
};

export default CreateTwinModal;
