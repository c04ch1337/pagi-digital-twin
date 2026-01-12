import React, { useState, useRef, useEffect, useMemo } from 'react';
import Prism from 'prismjs';
// Prism language components
import 'prismjs/components/prism-markdown';
import 'prismjs/components/prism-yaml';
import 'prismjs/components/prism-json';
import { Twin } from '../types';
import { AVAILABLE_TOOLS } from '../constants';

interface SettingsViewProps {
  twin: Twin;
  onSave: (twin: Twin) => void;
  onCancel: () => void;
}

const TACTICAL_BLUEPRINTS = [
  {
    id: 'red-team',
    name: 'Red Team Ops (MD)',
    lang: 'markdown',
    content: `# ADVERSARY EMULATION MANDATE\n## TARGET: INFRASTRUCTURE CORE\nYou are a high-tier Red Team Strategist. Your goal is to map exploitation chains.\n\n### METHODOLOGY\n1. **Reconnaissance**: Subdomain discovery and service fingerprinting.\n2. **Weaponization**: Identifying high-impact CVEs.\n3. **Persistence**: Planning lateral movement paths.`
  },
  {
    id: 'threat-hunter',
    name: 'Threat Hunter (YAML)',
    lang: 'yaml',
    content: `agent_id: "Hunter-Sentinel-01"\nmission_params:\n  focus: "lateral_movement"\n  priority: "high"\ndetection_logic:\n  - pattern: "powershell_enc_command"\n    severity: 9\n  - pattern: "unusual_rdp_auth"\n    severity: 7\n  - pattern: "dns_beaconing"\n    severity: 10`
  },
  {
    id: 'compliance',
    name: 'Policy Audit (JSON)',
    lang: 'json',
    content: `{\n  "agent_designation": "Policy Auditor",\n  "frameworks": ["NIST 800-53", "ISO 27001"],\n  "strict_mode": true,\n  "audit_sequence": [\n    "Verify ACL integrity",\n    "Check TLS/mTLS configuration",\n    "Validate MFA logs"\n  ]\n}`
  }
];

const SettingsView: React.FC<SettingsViewProps> = ({ twin, onSave, onCancel }) => {
  const [formData, setFormData] = useState<Twin>(JSON.parse(JSON.stringify(twin)));
  const [editorLanguage, setEditorLanguage] = useState<'markdown' | 'yaml' | 'json'>('markdown');
  const [isExampleMenuOpen, setIsExampleMenuOpen] = useState(false);
  const [isProviderDropdownOpen, setIsProviderDropdownOpen] = useState(false);
  const [isProcessingImage, setIsProcessingImage] = useState(false);
  
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const preRef = useRef<HTMLPreElement>(null);
  const exampleMenuRef = useRef<HTMLDivElement>(null);
  const providerMenuRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Sync scrolling between textarea and pre display layer
  const handleScroll = () => {
    if (textAreaRef.current && preRef.current) {
      preRef.current.scrollTop = textAreaRef.current.scrollTop;
      preRef.current.scrollLeft = textAreaRef.current.scrollLeft;
    }
  };

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (exampleMenuRef.current && !exampleMenuRef.current.contains(event.target as Node)) {
        setIsExampleMenuOpen(false);
      }
      if (providerMenuRef.current && !providerMenuRef.current.contains(event.target as Node)) {
        setIsProviderDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleToggleTool = (toolId: string) => {
    setFormData(prev => {
      const toolAccess = prev.settings.toolAccess.includes(toolId)
        ? prev.settings.toolAccess.filter(t => t !== toolId)
        : [...prev.settings.toolAccess, toolId];
      return { ...prev, settings: { ...prev.settings, toolAccess } };
    });
  };

  const handleToggleCodeGen = () => {
    setFormData(prev => ({
      ...prev,
      settings: {
        ...prev.settings,
        aiCodeGenerationEnabled: !prev.settings.aiCodeGenerationEnabled
      }
    }));
  };

  const handleLoadBlueprint = (blueprint: typeof TACTICAL_BLUEPRINTS[0]) => {
    setFormData(prev => ({ ...prev, systemPrompt: blueprint.content }));
    setEditorLanguage(blueprint.lang as any);
    setIsExampleMenuOpen(false);
  };

  const handleResetPrompt = () => {
    setFormData(prev => ({ ...prev, systemPrompt: twin.systemPrompt }));
  };

  const processFile = (file: File) => {
    if (!file) return;
    if (!file.type.startsWith('image/')) {
      alert("System Error: Tactical asset must be an image file.");
      return;
    }
    if (file.size > 2 * 1024 * 1024) {
      alert("System Limit: Image asset exceeds 2MB threshold.");
      return;
    }

    setIsProcessingImage(true);
    const reader = new FileReader();
    reader.onload = (e) => {
      const result = e.target?.result;
      if (typeof result === 'string') {
        setFormData(prev => ({ ...prev, avatar: result }));
      }
      setIsProcessingImage(false);
    };
    reader.readAsDataURL(file);
  };

  const handleAvatarChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      processFile(file);
    }
  };

  // Generate highlighted code using Prism
  const highlightedCode = useMemo(() => {
    const code = formData.systemPrompt || '';
    // Add a trailing space if the code ends with a newline to ensure scroll matching
    const textToHighlight = code + (code.endsWith('\n') ? ' ' : '');
    
    try {
      const grammar = Prism.languages[editorLanguage];
      if (grammar) {
        return Prism.highlight(textToHighlight, grammar, editorLanguage);
      }
    } catch (e) {
      console.warn("Prism Highlight Error:", e);
    }
    return textToHighlight;
  }, [formData.systemPrompt, editorLanguage]);

  useEffect(() => {
    handleScroll();
  }, [highlightedCode]);

  const providers = [
    { id: 'openrouter', name: 'OpenRouter', icon: 'rocket_launch', desc: 'Unified API Gateway Access (Default)' },
    { id: 'ollama', name: 'Ollama', icon: 'terminal', desc: 'Local Infrastructure Inference' },
    { id: 'gemini', name: 'Gemini', icon: 'google', desc: 'Google Native Multi-modal' },
    { id: 'openai', name: 'OpenAI', icon: 'psychology', desc: 'GPT-4o / o1 Architecture' },
    { id: 'anthropic', name: 'Anthropic', icon: 'neurology', desc: 'Claude 3.5 Sonnet Precision' },
    { id: 'llama', name: 'Llama', icon: 'adb', desc: 'Meta Specialized Weights' },
    { id: 'deepseek', name: 'DeepSeek', icon: 'troubleshoot', desc: 'High Efficiency Reasoning' },
    { id: 'mistral', name: 'Mistral', icon: 'air', desc: 'European Sovereign AI' },
    { id: 'grok', name: 'Grok', icon: 'smart_toy', desc: 'Real-time X.com Signals' }
  ];

  const currentProvider = providers.find(p => p.id === formData.settings.llmProvider) || providers[0];

  return (
    <div className="flex-1 bg-[#09090b] overflow-y-auto font-display selection:bg-indigo-500/30">
      <div className="max-w-[1100px] mx-auto py-8 px-6 text-zinc-300">
        <div className="flex items-center gap-2 mb-6">
          <button 
            onClick={onCancel}
            className="text-zinc-500 hover:text-indigo-400 text-sm font-medium transition-colors flex items-center gap-1"
          >
            <span className="material-symbols-outlined text-sm">smart_toy</span> Agents
          </button>
          <span className="text-zinc-700 text-sm">/</span>
          <span className="text-white text-sm font-medium">{formData.name}</span>
        </div>

        <div className="flex flex-wrap justify-between items-end gap-4 mb-8">
          <div className="flex flex-col gap-1">
            <h1 className="text-white text-4xl font-black leading-tight tracking-tight">Tactical Config</h1>
            <p className="text-zinc-500 text-sm font-mono uppercase tracking-[0.2em]">
              [NODE_ID: <span className="text-indigo-400">{formData.id}</span>]
            </p>
          </div>
          <div className="flex gap-3">
            <button 
              onClick={onCancel}
              className="flex items-center gap-2 rounded-lg h-10 px-6 bg-zinc-900 border border-zinc-800 text-zinc-400 hover:text-white transition-all text-xs font-bold"
            >
              Discard Changes
            </button>
            <button 
              onClick={() => onSave(formData)}
              className="flex items-center gap-2 rounded-lg h-10 px-8 bg-indigo-600 text-white hover:bg-indigo-500 transition-all text-xs font-bold shadow-lg shadow-indigo-600/20"
            >
              Commit Manifest
            </button>
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          <div className="lg:col-span-8 space-y-8">
            {/* Directive Logic Section with High-Precision Prism Syntax Highlighting */}
            <section className="bg-zinc-950 border border-zinc-800 rounded-2xl overflow-hidden flex flex-col shadow-2xl">
              <div className="flex items-center justify-between px-6 py-4 border-b border-zinc-900 bg-zinc-950/50 backdrop-blur-sm">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-indigo-500/10 rounded-lg">
                    <span className="material-symbols-outlined text-indigo-500 text-xl">terminal</span>
                  </div>
                  <h2 className="text-white text-lg font-bold">Directive Logic</h2>
                </div>
                
                <div className="flex items-center gap-3">
                  <button
                    onClick={handleResetPrompt}
                    className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-zinc-800 bg-zinc-900 text-zinc-500 text-[10px] font-black uppercase tracking-widest hover:text-rose-400 hover:border-rose-900/50 transition-all"
                    title="Restore default directive"
                  >
                    <span className="material-symbols-outlined text-sm">restart_alt</span>
                    Reset
                  </button>

                  <div className="relative" ref={exampleMenuRef}>
                    <button
                      onClick={() => setIsExampleMenuOpen(!isExampleMenuOpen)}
                      className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-zinc-800 bg-zinc-900 text-zinc-400 text-[10px] font-black uppercase tracking-widest hover:text-white transition-all"
                    >
                      <span className="material-symbols-outlined text-sm">lightbulb</span>
                      Blueprints
                    </button>
                    {isExampleMenuOpen && (
                      <div className="absolute top-full right-0 mt-2 w-64 bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl z-50 p-2 animate-in fade-in slide-in-from-top-2">
                        {TACTICAL_BLUEPRINTS.map(bp => (
                          <button
                            key={bp.id}
                            onClick={() => handleLoadBlueprint(bp)}
                            className="w-full text-left p-3 rounded-lg hover:bg-zinc-800 transition-colors group"
                          >
                            <div className="text-[11px] font-bold text-white group-hover:text-indigo-400 transition-colors uppercase tracking-tight">{bp.name}</div>
                            <div className="text-[9px] text-zinc-600 line-clamp-1 mt-1 font-mono">{bp.content.split('\n')[0]}</div>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>

                  <div className="flex items-center gap-1.5 p-1 bg-zinc-900 rounded-lg border border-zinc-800 shadow-inner">
                    {(['markdown', 'yaml', 'json'] as const).map((lang) => (
                      <button
                        key={lang}
                        onClick={() => setEditorLanguage(lang)}
                        className={`px-3 py-1 text-[10px] font-black uppercase tracking-widest rounded-md transition-all ${
                          editorLanguage === lang ? 'bg-indigo-600 text-white shadow-lg shadow-indigo-600/20' : 'text-zinc-500 hover:text-zinc-300'
                        }`}
                      >
                        {lang}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
              
              <div className="relative code-editor-bg flex h-[550px] font-mono">
                {/* Line numbers gutter */}
                <div className="w-12 shrink-0 flex flex-col items-center py-5 text-zinc-700 text-[11px] select-none bg-zinc-950/80 border-r border-zinc-800/50">
                  {(formData.systemPrompt || '').split('\n').map((_, i) => (
                    <span key={i} className="h-[20.8px] flex items-center justify-center w-full leading-[1.6]">
                      {String(i + 1).padStart(2, '0')}
                    </span>
                  ))}
                </div>
                
                {/* Syntax Highlighter Stack */}
                <div className="relative flex-1 overflow-hidden p-5">
                  <pre 
                    ref={preRef}
                    className={`absolute inset-5 pointer-events-none overflow-hidden language-${editorLanguage} scrollbar-hide`}
                    aria-hidden="true"
                  >
                    <code 
                      className={`language-${editorLanguage}`}
                      dangerouslySetInnerHTML={{ __html: highlightedCode }}
                    />
                  </pre>
                  
                  <textarea 
                    ref={textAreaRef}
                    className="code-textarea scrollbar-hide"
                    spellCheck="false"
                    value={formData.systemPrompt}
                    onScroll={handleScroll}
                    onChange={(e) => setFormData({ ...formData, systemPrompt: e.target.value })}
                    onKeyDown={(e) => {
                      if (e.key === 'Tab') {
                        e.preventDefault();
                        const start = e.currentTarget.selectionStart;
                        const end = e.currentTarget.selectionEnd;
                        const val = formData.systemPrompt;
                        setFormData({
                          ...formData,
                          systemPrompt: val.substring(0, start) + '  ' + val.substring(end)
                        });
                        // Set cursor position back after state update
                        setTimeout(() => {
                           if (textAreaRef.current) {
                             textAreaRef.current.selectionStart = textAreaRef.current.selectionEnd = start + 2;
                           }
                        }, 0);
                      }
                    }}
                  />
                </div>
              </div>
              
              <div className="bg-zinc-950 border-t border-zinc-900 px-6 py-3 flex items-center justify-between text-zinc-600">
                <div className="flex items-center gap-8">
                  <span className="text-[9px] font-black uppercase tracking-[0.2em] flex items-center gap-2">
                    <span className="material-symbols-outlined text-[14px] text-zinc-700">file_download_done</span>
                    {new Blob([formData.systemPrompt]).size} BYTES
                  </span>
                  <span className="text-[9px] font-black uppercase tracking-[0.2em] flex items-center gap-2">
                    <span className="material-symbols-outlined text-[14px] text-zinc-700">format_list_numbered</span>
                    {(formData.systemPrompt || '').split('\n').length} LINES
                  </span>
                </div>
                <div className="flex items-center gap-3">
                   <span className="text-[9px] font-black uppercase tracking-[0.2em] text-zinc-500">Syntax Layer: {editorLanguage.toUpperCase()}</span>
                   <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.5)]" />
                </div>
              </div>
            </section>

            {/* Tactical Identity */}
            <section className="bg-zinc-950 border border-zinc-800 rounded-2xl p-6 shadow-2xl relative overflow-hidden group/card">
              <div className="absolute top-0 right-0 w-32 h-32 bg-indigo-600/5 blur-[60px] rounded-full -mr-16 -mt-16 group-hover/card:bg-indigo-600/10 transition-colors duration-500" />
              <div className="flex items-center gap-3 mb-6 relative">
                <div className="p-2 bg-indigo-500/10 rounded-lg">
                  <span className="material-symbols-outlined text-indigo-500 text-xl">fingerprint</span>
                </div>
                <h2 className="text-white text-lg font-bold">Tactical Identity</h2>
              </div>
              <div className="flex flex-col md:flex-row gap-8 items-start relative">
                <div className="flex flex-col items-center gap-4">
                  <div 
                    className="relative group cursor-pointer transition-all duration-300 rounded-2xl overflow-hidden border-2 border-zinc-800 bg-zinc-900 h-28 w-28 flex items-center justify-center shadow-2xl shadow-black/50"
                    onClick={() => fileInputRef.current?.click()}
                  >
                    {isProcessingImage ? (
                      <div className="w-6 h-6 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" />
                    ) : (
                      <img src={formData.avatar} alt={formData.name} className="size-full object-cover grayscale group-hover:grayscale-0 transition-all duration-500" />
                    )}
                    <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity">
                      <span className="material-symbols-outlined text-white text-xl">add_a_photo</span>
                      <span className="text-[8px] font-bold text-white uppercase mt-1">Upload</span>
                    </div>
                  </div>
                  <input 
                    type="file" 
                    ref={fileInputRef} 
                    className="hidden" 
                    accept="image/*" 
                    onChange={handleAvatarChange}
                  />
                </div>

                <div className="flex-1 grid grid-cols-1 md:grid-cols-2 gap-6 w-full">
                  <label className="flex flex-col gap-2">
                    <span className="text-zinc-500 text-[10px] font-black uppercase tracking-widest">Codename</span>
                    <input 
                      className="w-full rounded-xl text-zinc-100 border border-zinc-800 bg-zinc-900/50 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/20 h-11 px-4 transition-all outline-none text-sm font-medium"
                      value={formData.name}
                      onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                    />
                  </label>
                  <label className="flex flex-col gap-2">
                    <span className="text-zinc-500 text-[10px] font-black uppercase tracking-widest">Designation</span>
                    <input 
                      className="w-full rounded-xl text-zinc-100 border border-zinc-800 bg-zinc-900/50 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/20 h-11 px-4 transition-all outline-none text-sm font-medium"
                      value={formData.role}
                      onChange={(e) => setFormData({ ...formData, role: e.target.value })}
                    />
                  </label>
                  <label className="flex flex-col gap-2 md:col-span-2">
                    <span className="text-zinc-500 text-[10px] font-black uppercase tracking-widest">Mission Summary</span>
                    <input 
                      className="w-full rounded-xl text-zinc-100 border border-zinc-800 bg-zinc-900/50 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/20 h-11 px-4 transition-all outline-none text-sm font-medium"
                      value={formData.description}
                      onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                    />
                  </label>
                </div>
              </div>
            </section>
          </div>

          <div className="lg:col-span-4 space-y-6">
            {/* Neural Core Selection */}
            <section className="bg-zinc-950 border border-zinc-800 rounded-2xl p-6 shadow-2xl">
              <div className="flex items-center gap-3 mb-6">
                <div className="p-2 bg-indigo-500/10 rounded-lg">
                  <span className="material-symbols-outlined text-indigo-500 text-xl">memory</span>
                </div>
                <h2 className="text-white text-lg font-bold">Neural Core</h2>
              </div>
              <div className="space-y-6">
                <div className="relative" ref={providerMenuRef}>
                   <span className="text-zinc-500 text-[10px] font-black uppercase tracking-widest mb-3 block">LLM Provider Selection</span>
                   <button
                    onClick={() => setIsProviderDropdownOpen(!isProviderDropdownOpen)}
                    className="w-full flex items-center justify-between p-3 rounded-xl border border-zinc-800 bg-zinc-900/50 hover:border-indigo-500/50 transition-all text-left group"
                   >
                     <div className="flex items-center gap-3">
                        <span className="material-symbols-outlined text-indigo-400">
                          {currentProvider.icon}
                        </span>
                        <div className="flex flex-col">
                           <span className="text-[11px] font-bold text-white uppercase tracking-tight">{currentProvider.name}</span>
                           <span className="text-[9px] text-zinc-600 font-bold uppercase tracking-widest">Active Link</span>
                        </div>
                     </div>
                     <span className={`material-symbols-outlined text-zinc-500 transition-transform duration-300 ${isProviderDropdownOpen ? 'rotate-180' : ''}`}>
                       expand_more
                     </span>
                   </button>

                   {isProviderDropdownOpen && (
                     <div className="absolute top-full left-0 right-0 mt-2 bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl z-[60] p-1.5 animate-in fade-in slide-in-from-top-2">
                        {providers.map(p => (
                          <button
                            key={p.id}
                            onClick={() => {
                              setFormData({ ...formData, settings: { ...formData.settings, llmProvider: p.id as any } });
                              setIsProviderDropdownOpen(false);
                            }}
                            className={`w-full flex items-center gap-3 p-2.5 rounded-lg transition-all text-left group ${
                              formData.settings.llmProvider === p.id ? 'bg-indigo-600/10' : 'hover:bg-zinc-800'
                            }`}
                          >
                            <span className={`material-symbols-outlined text-lg ${formData.settings.llmProvider === p.id ? 'text-indigo-400' : 'text-zinc-600 group-hover:text-zinc-400'}`}>
                              {p.icon}
                            </span>
                            <div className="flex flex-col">
                              <span className={`text-[10px] font-bold uppercase tracking-tight ${formData.settings.llmProvider === p.id ? 'text-indigo-400' : 'text-zinc-300'}`}>
                                {p.name}
                              </span>
                              <span className="text-[8px] text-zinc-600 group-hover:text-zinc-500 uppercase tracking-widest font-black">{p.desc}</span>
                            </div>
                            {formData.settings.llmProvider === p.id && (
                              <span className="material-symbols-outlined text-indigo-400 text-sm ml-auto">check</span>
                            )}
                          </button>
                        ))}
                     </div>
                   )}
                </div>

                {/* API Key Input - Secure View */}
                {formData.settings.llmProvider !== 'gemini' && (
                  <div className="space-y-2 animate-in fade-in slide-in-from-top-1 duration-300">
                    <span className="text-zinc-500 text-[10px] font-black uppercase tracking-widest">Infrastructure Key</span>
                    <div className="relative">
                      <input 
                        type="password"
                        placeholder="sk-..."
                        className="w-full rounded-xl text-zinc-100 border border-zinc-800 bg-zinc-900/50 focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/20 h-11 pl-10 pr-4 transition-all outline-none text-xs font-mono"
                        value={formData.settings.apiKey || ''}
                        onChange={(e) => setFormData({ 
                          ...formData, 
                          settings: { ...formData.settings, apiKey: e.target.value } 
                        })}
                      />
                      <span className="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600 text-sm">key</span>
                    </div>
                  </div>
                )}

                <div className="space-y-5 pt-4 border-t border-zinc-800/50">
                   <div className="space-y-3">
                      <div className="flex justify-between items-center">
                        <span className="text-zinc-500 text-[10px] font-black uppercase tracking-widest">Logic Variance</span>
                        <span className="text-xs font-mono text-indigo-400 font-bold">{formData.settings.temperature.toFixed(2)}</span>
                      </div>
                      <input 
                        type="range" min="0" max="1.5" step="0.05"
                        value={formData.settings.temperature}
                        onChange={(e) => setFormData({ ...formData, settings: { ...formData.settings, temperature: parseFloat(e.target.value) } })}
                        className="w-full h-1.5 bg-zinc-900 rounded-lg appearance-none cursor-pointer accent-indigo-600"
                      />
                   </div>

                   <div className="space-y-3">
                      <div className="flex justify-between items-center">
                        <span className="text-zinc-500 text-[10px] font-black uppercase tracking-widest">Context Shards</span>
                        <span className="text-xs font-mono text-indigo-400 font-bold">{formData.settings.tokenLimit}K</span>
                      </div>
                      <input 
                        type="range" min="16" max="128" step="16"
                        value={formData.settings.tokenLimit}
                        onChange={(e) => setFormData({ ...formData, settings: { ...formData.settings, tokenLimit: parseInt(e.target.value) } })}
                        className="w-full h-1.5 bg-zinc-900 rounded-lg appearance-none cursor-pointer accent-indigo-600"
                      />
                   </div>
                </div>
              </div>
            </section>

            {/* Capability Permissions & Defensive Policies */}
            <section className="bg-zinc-950 border border-zinc-800 rounded-2xl p-6 shadow-2xl">
              <div className="flex items-center gap-3 mb-6">
                <div className="p-2 bg-indigo-500/10 rounded-lg">
                  <span className="material-symbols-outlined text-indigo-500 text-xl">shield</span>
                </div>
                <h2 className="text-white text-lg font-bold">Policy Matrix</h2>
              </div>
              
              <div className="space-y-4">
                {/* Global Defensive Policy Toggles */}
                <div className="space-y-2">
                  <span className="text-zinc-600 text-[9px] font-black uppercase tracking-widest mb-1 block">Defensive Policies</span>
                  <button 
                    onClick={handleToggleCodeGen}
                    className={`w-full flex items-center justify-between p-3 rounded-xl border transition-all text-left ${
                      formData.settings.aiCodeGenerationEnabled 
                        ? 'bg-indigo-600/10 border-indigo-600/30 text-white' 
                        : 'bg-zinc-900/50 border-zinc-800 text-zinc-500 opacity-60'
                    }`}
                  >
                     <div className="flex items-center gap-3">
                       <span className={`material-symbols-outlined text-sm ${formData.settings.aiCodeGenerationEnabled ? 'text-indigo-400' : 'text-zinc-600'}`}>
                         code
                       </span>
                       <span className="text-[11px] font-bold uppercase tracking-tight">AI Code Generation</span>
                     </div>
                     <div className={`w-8 h-4 rounded-full relative transition-colors ${formData.settings.aiCodeGenerationEnabled ? 'bg-indigo-600' : 'bg-zinc-800'}`}>
                        <div className={`absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all ${formData.settings.aiCodeGenerationEnabled ? 'right-0.5' : 'left-0.5'}`} />
                     </div>
                  </button>
                </div>

                {/* Tactical Tools */}
                <div className="space-y-2">
                  <span className="text-zinc-600 text-[9px] font-black uppercase tracking-widest mb-1 block">Tactical Tools</span>
                  {AVAILABLE_TOOLS.map(tool => (
                    <button 
                      key={tool.id}
                      onClick={() => handleToggleTool(tool.id)}
                      className={`flex items-center gap-4 p-3 rounded-xl border transition-all text-left group/tool ${
                        formData.settings.toolAccess.includes(tool.id) 
                          ? 'bg-indigo-600/5 border-indigo-600/30' 
                          : 'bg-zinc-900/50 border-zinc-800 opacity-40 grayscale'
                      }`}
                    >
                       <span className={`material-symbols-outlined text-sm ${formData.settings.toolAccess.includes(tool.id) ? 'text-indigo-400' : 'text-zinc-600'}`}>
                         {tool.icon}
                       </span>
                       <div className="flex-1">
                         <div className="text-[11px] font-bold text-zinc-100 uppercase tracking-tight">{tool.name}</div>
                       </div>
                       <span className="material-symbols-outlined text-lg text-indigo-500">
                         {formData.settings.toolAccess.includes(tool.id) ? 'check_circle' : 'circle'}
                       </span>
                    </button>
                  ))}
                </div>
              </div>
            </section>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SettingsView;