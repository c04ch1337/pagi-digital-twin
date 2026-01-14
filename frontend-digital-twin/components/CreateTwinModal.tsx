
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
        llmProvider: 'openrouter',
        // Fix: Added missing temperature and topP properties required by TwinSettings interface
        temperature: 0.7,
        topP: 0.95
      }
    };

    onSave(newTwin);
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-[rgb(var(--bg-primary-rgb)/0.95)] backdrop-blur-md animate-in fade-in duration-200">
      <div className="w-full max-w-2xl bg-[rgb(var(--surface-rgb)/0.9)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        <div className="p-6 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center justify-between bg-[var(--bg-secondary)]">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-[var(--bg-steel)]">add_to_drive</span>
            <h2 className="text-xl font-bold text-[var(--text-primary)] font-display">Register New Tactical Agent</h2>
          </div>
          <button 
            onClick={onClose}
            className="text-[var(--text-secondary)] hover:text-[var(--bg-steel)] transition-colors"
          >
            <span className="material-symbols-outlined">close</span>
          </button>
        </div>

        <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto p-6 space-y-6 bg-[rgb(var(--surface-rgb)/0.6)]">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <label className="flex flex-col gap-2">
              <span className="text-[var(--text-secondary)] text-xs font-bold uppercase tracking-widest">Codename</span>
              <input 
                required
                className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg h-11 px-4 text-[var(--text-primary)] focus:border-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)] outline-none transition-all text-sm"
                placeholder="e.g. Shadow Vector"
                value={formData.name}
                onChange={e => setFormData({ ...formData, name: e.target.value })}
              />
            </label>
            <label className="flex flex-col gap-2">
              <span className="text-[var(--text-secondary)] text-xs font-bold uppercase tracking-widest">Tactical Role</span>
              <input 
                required
                className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg h-11 px-4 text-[var(--text-primary)] focus:border-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)] outline-none transition-all text-sm"
                placeholder="e.g. Signal Interceptor"
                value={formData.role}
                onChange={e => setFormData({ ...formData, role: e.target.value })}
              />
            </label>
            <label className="flex flex-col gap-2 md:col-span-2">
              <span className="text-[var(--text-secondary)] text-xs font-bold uppercase tracking-widest">Mission Summary</span>
              <input 
                className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg h-11 px-4 text-[var(--text-primary)] focus:border-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)] outline-none transition-all text-sm"
                placeholder="Brief description of the node's function"
                value={formData.description}
                onChange={e => setFormData({ ...formData, description: e.target.value })}
              />
            </label>
          </div>

          <label className="flex flex-col gap-2">
            <span className="text-[var(--text-secondary)] text-xs font-bold uppercase tracking-widest">Directive Logic (System Prompt)</span>
            <textarea 
              rows={6}
              className="bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4 text-[var(--text-primary)] font-mono text-sm focus:border-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)] outline-none transition-all resize-none"
              placeholder="# OPERATIONAL MANDATE\nYou are a high-performance agent designed for..."
              value={formData.systemPrompt}
              onChange={e => setFormData({ ...formData, systemPrompt: e.target.value })}
            />
          </label>

          <div className="p-4 bg-[rgb(var(--bg-muted-rgb)/0.1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl">
            <div className="flex items-start gap-3">
              <span className="material-symbols-outlined text-[var(--bg-steel)] text-sm mt-0.5">info</span>
              <p className="text-[11px] text-[var(--text-secondary)] leading-relaxed">
                By default, new agents are initialized in <strong>Safe Mode</strong> with restricted tool access. 
                You can adjust defensive policies in the agent configuration view after registration.
              </p>
            </div>
          </div>
        </form>

        <div className="p-6 border-t border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)] flex justify-end gap-3">
          <button 
            type="button"
            onClick={onClose}
            className="px-6 py-2 rounded-lg text-sm font-bold text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-muted)] transition-all"
          >
            Cancel
          </button>
          <button 
            type="submit"
            onClick={handleSubmit}
            className="bg-[var(--bg-steel)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] text-[var(--text-on-accent)] px-8 py-2 rounded-lg text-sm font-bold shadow-lg shadow-[rgb(var(--bg-steel-rgb)/0.2)] transition-all flex items-center gap-2"
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
