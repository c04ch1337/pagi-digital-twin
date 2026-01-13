import React, { useState, useRef, useEffect, useMemo } from 'react';
import Prism from 'prismjs';
// Prism language components
import 'prismjs/components/prism-markdown';
import 'prismjs/components/prism-yaml';
import 'prismjs/components/prism-json';
import { Twin } from '../types';
import { AVAILABLE_TOOLS } from '../constants';
import HoverTooltip from './HoverTooltip';
import { uploadAsset } from '../services/assetService';

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
  const [uploading, setUploading] = useState<string | null>(null);
  const [uploadStatus, setUploadStatus] = useState<Record<string, string>>({});
  
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

  const handleAssetUpload = async (file: File, assetType: 'logo' | 'favicon' | 'favicon-png') => {
    setUploading(assetType);
    setUploadStatus(prev => ({ ...prev, [assetType]: 'Uploading...' }));
    try {
      const result = await uploadAsset(file, assetType);
      setUploadStatus(prev => ({ ...prev, [assetType]: 'Uploaded successfully! Reload page to see changes.' }));
      // Optionally reload after a delay
      setTimeout(() => {
        setUploadStatus(prev => ({ ...prev, [assetType]: '' }));
      }, 3000);
    } catch (error) {
      setUploadStatus(prev => ({ 
        ...prev, 
        [assetType]: `Error: ${error instanceof Error ? error.message : 'Upload failed'}` 
      }));
    } finally {
      setUploading(null);
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
    <div className="flex-1 bg-[#9EC9D9] overflow-y-auto font-display selection:bg-[#5381A5]/30">
      <div className="max-w-[1100px] mx-auto py-8 px-6 text-[#0b1b2b] text-[14px]">
        <div className="flex items-center gap-2 mb-6">
          <button 
            onClick={onCancel}
            className="text-[#163247] hover:text-[#5381A5] text-sm font-medium transition-colors flex items-center gap-1"
          >
            <span className="material-symbols-outlined text-sm">smart_toy</span> Agents
          </button>
          <span className="text-[#163247]/60 text-sm">/</span>
          <span className="text-[#0b1b2b] text-sm font-medium">{formData.name}</span>
        </div>

        <div className="flex flex-wrap justify-between items-end gap-4 mb-8">
          <div className="flex flex-col gap-1">
            <h1 className="text-[#0b1b2b] text-4xl font-black leading-tight tracking-tight">Tactical Config</h1>
            <p className="text-[#163247] text-sm font-mono uppercase tracking-[0.2em]">
              [NODE_ID: <span className="text-[#5381A5]">{formData.id}</span>]
            </p>
          </div>
          <div className="flex gap-3">
            <button 
              onClick={onCancel}
              className="flex items-center gap-2 rounded-lg h-10 px-6 bg-white/35 border border-[#5381A5]/35 text-[#0b1b2b] hover:bg-[#78A2C2] hover:text-[#0b1b2b] transition-all text-[13px] font-bold"
            >
              Discard Changes
            </button>
            <button 
              onClick={() => onSave(formData)}
              className="flex items-center gap-2 rounded-lg h-10 px-8 bg-[#5381A5] text-white hover:bg-[#437091] transition-all text-[13px] font-bold shadow-lg shadow-[#5381A5]/20"
            >
              Commit Manifest
            </button>
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          <div className="lg:col-span-8 space-y-8">
            {/* Directive Logic Section with High-Precision Prism Syntax Highlighting */}
            <section className="bg-white/50 border border-[#5381A5]/35 rounded-2xl overflow-hidden flex flex-col shadow-2xl">
              <div className="flex items-center justify-between px-6 py-4 border-b border-[#5381A5]/20 bg-white/30 backdrop-blur-sm">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-white/40 rounded-lg border border-[#5381A5]/20">
                    <span className="material-symbols-outlined text-[#5381A5] text-xl">terminal</span>
                  </div>
                  <h2 className="text-[#0b1b2b] text-lg font-bold">Directive Logic</h2>
                </div>
                
                <div className="flex items-center gap-3">
                  <HoverTooltip
                    title="Reset Directive"
                    description="Revert the directive text back to the agent’s saved baseline (discarding any unsaved edits in the editor)."
                  >
                    <button
                      onClick={handleResetPrompt}
                      className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-[#5381A5]/35 bg-white/35 text-[#0b1b2b] text-[11px] font-black uppercase tracking-widest hover:text-rose-700 hover:border-rose-500/40 transition-all"
                      title="Restore default directive"
                    >
                      <span className="material-symbols-outlined text-sm">restart_alt</span>
                      Reset
                    </button>
                  </HoverTooltip>

                  <div className="relative" ref={exampleMenuRef}>
                    <HoverTooltip
                      title="Blueprints"
                      description="Insert a prebuilt directive template (Markdown/YAML/JSON) as a starting point for the agent’s mission logic."
                    >
                      <button
                        onClick={() => setIsExampleMenuOpen(!isExampleMenuOpen)}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-[#5381A5]/35 bg-white/35 text-[#0b1b2b] text-[11px] font-black uppercase tracking-widest hover:bg-[#78A2C2] transition-all"
                      >
                        <span className="material-symbols-outlined text-sm">lightbulb</span>
                        Blueprints
                      </button>
                    </HoverTooltip>
                    {isExampleMenuOpen && (
                      <div className="absolute top-full right-0 mt-2 w-64 bg-[#90C3EA] border border-[#5381A5]/35 rounded-xl shadow-2xl z-50 p-2 animate-in fade-in slide-in-from-top-2">
                        {TACTICAL_BLUEPRINTS.map(bp => (
                          <button
                            key={bp.id}
                            onClick={() => handleLoadBlueprint(bp)}
                            className="w-full text-left p-3 rounded-lg hover:bg-[#78A2C2] transition-colors group"
                          >
                            <div className="text-[11px] font-bold text-[#0b1b2b] group-hover:text-[#5381A5] transition-colors uppercase tracking-tight">{bp.name}</div>
                            <div className="text-[10px] text-[#163247] line-clamp-1 mt-1 font-mono">{bp.content.split('\n')[0]}</div>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>

                  <div className="flex items-center gap-1.5 p-1 bg-white/35 rounded-lg border border-[#5381A5]/35 shadow-inner">
                    {(['markdown', 'yaml', 'json'] as const).map((lang) => (
                      <HoverTooltip
                        key={lang}
                        title={`Syntax: ${lang.toUpperCase()}`}
                        description="Switches the syntax highlighting mode for the directive editor."
                      >
                        <button
                          onClick={() => setEditorLanguage(lang)}
                          className={`px-3 py-1 text-[11px] font-black uppercase tracking-widest rounded-md transition-all ${
                            editorLanguage === lang
                              ? 'bg-[#5381A5] text-white shadow-lg shadow-[#5381A5]/20'
                              : 'text-[#163247] hover:text-[#0b1b2b]'
                          }`}
                        >
                          {lang}
                        </button>
                      </HoverTooltip>
                    ))}
                  </div>
                </div>
              </div>
              
              <div className="relative code-editor-bg flex h-[550px] font-mono">
                {/* Line numbers gutter */}
                <div className="w-12 shrink-0 flex flex-col items-center py-5 text-[#0b1b2b]/70 text-[12px] font-semibold select-none bg-white/35 border-r border-[#5381A5]/25">
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
              
              <div className="bg-white/35 border-t border-[#5381A5]/25 px-6 py-3 flex items-center justify-between text-[#163247]">
                <div className="flex items-center gap-8">
                    <span className="text-[10px] font-black uppercase tracking-[0.2em] flex items-center gap-2">
                    <span className="material-symbols-outlined text-[14px] text-[#5381A5]">file_download_done</span>
                    {new Blob([formData.systemPrompt]).size} BYTES
                  </span>
                  <span className="text-[10px] font-black uppercase tracking-[0.2em] flex items-center gap-2">
                    <span className="material-symbols-outlined text-[14px] text-[#5381A5]">format_list_numbered</span>
                    {(formData.systemPrompt || '').split('\n').length} LINES
                  </span>
                </div>
                <div className="flex items-center gap-3">
                   <span className="text-[10px] font-black uppercase tracking-[0.2em] text-[#163247]">Syntax Layer: {editorLanguage.toUpperCase()}</span>
                   <div className="w-1.5 h-1.5 rounded-full bg-[#5381A5] animate-pulse shadow-[0_0_8px_rgba(83,129,165,0.45)]" />
                </div>
              </div>
            </section>

            {/* Tactical Identity */}
            <section className="bg-white/40 border border-[#5381A5]/30 rounded-2xl p-6 shadow-2xl relative overflow-hidden group/card">
              <div className="absolute top-0 right-0 w-32 h-32 bg-[#5381A5]/10 blur-[60px] rounded-full -mr-16 -mt-16 group-hover/card:bg-[#5381A5]/20 transition-colors duration-500" />
              <div className="flex items-center gap-3 mb-6 relative">
                <div className="p-2 bg-white/40 rounded-lg border border-[#5381A5]/20">
                  <span className="material-symbols-outlined text-[#5381A5] text-xl">fingerprint</span>
                </div>
                <h2 className="text-[#0b1b2b] text-lg font-bold">Tactical Identity</h2>
              </div>
              <div className="flex flex-col md:flex-row gap-8 items-start relative">
                <div className="flex flex-col items-center gap-4">
                  <div 
                    className="relative group cursor-pointer transition-all duration-300 rounded-2xl overflow-hidden border-2 border-[#5381A5]/30 bg-white/30 h-28 w-28 flex items-center justify-center shadow-2xl shadow-black/20"
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
                    <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Codename</span>
                    <input 
                      className="w-full rounded-xl text-[#0b1b2b] border border-[#5381A5]/30 bg-white/30 focus:border-[#5381A5] focus:ring-1 focus:ring-[#5381A5]/20 h-11 px-4 transition-all outline-none text-sm font-medium"
                      value={formData.name}
                      onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                    />
                  </label>
                  <label className="flex flex-col gap-2">
                    <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Designation</span>
                    <input 
                      className="w-full rounded-xl text-[#0b1b2b] border border-[#5381A5]/30 bg-white/30 focus:border-[#5381A5] focus:ring-1 focus:ring-[#5381A5]/20 h-11 px-4 transition-all outline-none text-sm font-medium"
                      value={formData.role}
                      onChange={(e) => setFormData({ ...formData, role: e.target.value })}
                    />
                  </label>
                  <label className="flex flex-col gap-2 md:col-span-2">
                    <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Mission Summary</span>
                    <input 
                      className="w-full rounded-xl text-[#0b1b2b] border border-[#5381A5]/30 bg-white/30 focus:border-[#5381A5] focus:ring-1 focus:ring-[#5381A5]/20 h-11 px-4 transition-all outline-none text-sm font-medium"
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
            <section className="bg-white/40 border border-[#5381A5]/30 rounded-2xl p-6 shadow-2xl">
              <div className="flex items-center gap-3 mb-6">
                <div className="p-2 bg-white/40 rounded-lg border border-[#5381A5]/20">
                  <span className="material-symbols-outlined text-[#5381A5] text-xl">memory</span>
                </div>
                <h2 className="text-[#0b1b2b] text-lg font-bold">Neural Core</h2>
              </div>
              <div className="space-y-6">
                <div className="relative" ref={providerMenuRef}>
                   <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest mb-3 block">LLM Provider Selection</span>
                   <HoverTooltip
                     title="LLM Provider"
                     description="Select the language model backend this agent uses for planning. Provider choice impacts latency, capability, and cost."
                   >
                     <button
                       onClick={() => setIsProviderDropdownOpen(!isProviderDropdownOpen)}
                       className="w-full flex items-center justify-between p-3 rounded-xl border border-[#5381A5]/30 bg-white/30 hover:border-[#5381A5]/60 transition-all text-left group"
                     >
                       <div className="flex items-center gap-3">
                          <span className="material-symbols-outlined text-[#5381A5]">
                            {currentProvider.icon}
                          </span>
                          <div className="flex flex-col">
                             <span className="text-[11px] font-bold text-[#0b1b2b] uppercase tracking-tight">{currentProvider.name}</span>
                             <span className="text-[9px] text-[#163247] font-bold uppercase tracking-widest">Active Link</span>
                          </div>
                       </div>
                       <span className={`material-symbols-outlined text-[#163247] transition-transform duration-300 ${isProviderDropdownOpen ? 'rotate-180' : ''}`}>
                         expand_more
                       </span>
                     </button>
                   </HoverTooltip>

                   {isProviderDropdownOpen && (
                     <div className="absolute top-full left-0 right-0 mt-2 bg-[#90C3EA] border border-[#5381A5]/30 rounded-xl shadow-2xl z-[60] p-1.5 animate-in fade-in slide-in-from-top-2">
                        {providers.map(p => (
                          <button
                            key={p.id}
                            onClick={() => {
                              setFormData({ ...formData, settings: { ...formData.settings, llmProvider: p.id as any } });
                              setIsProviderDropdownOpen(false);
                            }}
                            className={`w-full flex items-center gap-3 p-2.5 rounded-lg transition-all text-left group ${
                              formData.settings.llmProvider === p.id ? 'bg-white/40 border border-[#5381A5]/30' : 'hover:bg-[#78A2C2]'
                            }`}
                          >
                            <span className={`material-symbols-outlined text-lg ${formData.settings.llmProvider === p.id ? 'text-[#5381A5]' : 'text-[#163247] group-hover:text-[#0b1b2b]'}`}>
                              {p.icon}
                            </span>
                            <div className="flex flex-col">
                              <span className={`text-[10px] font-bold uppercase tracking-tight ${formData.settings.llmProvider === p.id ? 'text-[#5381A5]' : 'text-[#0b1b2b]'}`}>
                                {p.name}
                              </span>
                              <span className="text-[8px] text-[#163247] group-hover:text-[#0b1b2b] uppercase tracking-widest font-black">{p.desc}</span>
                            </div>
                            {formData.settings.llmProvider === p.id && (
                              <span className="material-symbols-outlined text-[#5381A5] text-sm ml-auto">check</span>
                            )}
                          </button>
                        ))}
                     </div>
                   )}
                </div>

                {/* API Key Input - Secure View */}
                {formData.settings.llmProvider !== 'gemini' && (
                  <div className="space-y-2 animate-in fade-in slide-in-from-top-1 duration-300">
                    <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Infrastructure Key</span>
                    <div className="relative">
                      <HoverTooltip
                        title="API Key"
                        description="Credential used to authenticate with the selected provider (stored in this agent config). Keep this secret."
                      >
                        <input 
                          type="password"
                          placeholder="sk-..."
                          className="w-full rounded-xl text-[#0b1b2b] border border-[#5381A5]/30 bg-white/30 focus:border-[#5381A5] focus:ring-1 focus:ring-[#5381A5]/20 h-11 pl-10 pr-4 transition-all outline-none text-xs font-mono"
                          value={formData.settings.apiKey || ''}
                          onChange={(e) => setFormData({ 
                            ...formData, 
                            settings: { ...formData.settings, apiKey: e.target.value } 
                          })}
                        />
                      </HoverTooltip>
                      <span className="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-[#5381A5] text-sm">key</span>
                    </div>
                  </div>
                )}

                <div className="space-y-5 pt-4 border-t border-[#5381A5]/20">
                   <div className="space-y-3">
                      <div className="flex justify-between items-center">
                        <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Logic Variance</span>
                        <span className="text-xs font-mono text-[#5381A5] font-bold">{formData.settings.temperature.toFixed(2)}</span>
                      </div>
                      <HoverTooltip
                        title="Logic Variance (Temperature)"
                        description="Higher values increase creativity/variance; lower values make the agent more deterministic and policy-following."
                      >
                        <input 
                          type="range" min="0" max="1.5" step="0.05"
                          value={formData.settings.temperature}
                          onChange={(e) => setFormData({ ...formData, settings: { ...formData.settings, temperature: parseFloat(e.target.value) } })}
                          className="w-full h-1.5 bg-white/40 rounded-lg appearance-none cursor-pointer accent-[#5381A5]"
                        />
                      </HoverTooltip>
                   </div>

                   <div className="space-y-3">
                      <div className="flex justify-between items-center">
                        <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Context Shards</span>
                        <span className="text-xs font-mono text-[#5381A5] font-bold">{formData.settings.tokenLimit}K</span>
                      </div>
                      <HoverTooltip
                        title="Context Shards (Token Limit)"
                        description="Maximum context window size used for planning. Higher limits improve recall but may increase latency/cost."
                      >
                        <input 
                          type="range" min="16" max="128" step="16"
                          value={formData.settings.tokenLimit}
                          onChange={(e) => setFormData({ ...formData, settings: { ...formData.settings, tokenLimit: parseInt(e.target.value) } })}
                          className="w-full h-1.5 bg-white/40 rounded-lg appearance-none cursor-pointer accent-[#5381A5]"
                        />
                      </HoverTooltip>
                   </div>
                </div>
              </div>
            </section>

            {/* Capability Permissions & Defensive Policies */}
            <section className="bg-white/40 border border-[#5381A5]/30 rounded-2xl p-6 shadow-2xl">
              <div className="flex items-center gap-3 mb-6">
                <div className="p-2 bg-white/40 rounded-lg border border-[#5381A5]/20">
                  <span className="material-symbols-outlined text-[#5381A5] text-xl">shield</span>
                </div>
                <h2 className="text-[#0b1b2b] text-lg font-bold">Policy Matrix</h2>
              </div>
              
              <div className="space-y-4">
                {/* Global Defensive Policy Toggles */}
                <div className="space-y-2">
                  <span className="text-[#163247] text-[9px] font-black uppercase tracking-widest mb-1 block">Defensive Policies</span>
                  <HoverTooltip
                    title="AI Code Generation"
                    description="Controls whether the agent is allowed to propose creating new tools (high privilege). Disable to prevent build requests."
                  >
                    <button 
                      onClick={handleToggleCodeGen}
                      className={`w-full flex items-center justify-between p-3 rounded-xl border transition-all text-left ${
                        formData.settings.aiCodeGenerationEnabled 
                          ? 'bg-white/40 border-[#5381A5]/40 text-[#0b1b2b]' 
                          : 'bg-white/20 border-[#5381A5]/20 text-[#163247] opacity-60'
                      }`}
                    >
                      <div className="flex items-center gap-3">
                        <span className={`material-symbols-outlined text-sm ${formData.settings.aiCodeGenerationEnabled ? 'text-[#5381A5]' : 'text-[#163247]'}`}>
                          code
                        </span>
                        <span className="text-[11px] font-bold uppercase tracking-tight">AI Code Generation</span>
                      </div>
                      <div className={`w-8 h-4 rounded-full relative transition-colors ${formData.settings.aiCodeGenerationEnabled ? 'bg-[#5381A5]' : 'bg-white/40'}`}>
                        <div className={`absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all ${formData.settings.aiCodeGenerationEnabled ? 'right-0.5' : 'left-0.5'}`} />
                      </div>
                    </button>
                  </HoverTooltip>
                </div>

                {/* Tactical Tools */}
                <div className="space-y-2">
                  <span className="text-[#163247] text-[9px] font-black uppercase tracking-widest mb-1 block">Tactical Tools</span>
                  {AVAILABLE_TOOLS.map(tool => (
                    <HoverTooltip
                      key={tool.id}
                      title={`Tool Permission: ${tool.name}`}
                      description={`Enable/disable this tool for the agent. When enabled, the Orchestrator may request approval to execute it.`}
                    >
                      <button 
                        onClick={() => handleToggleTool(tool.id)}
                        className={`flex items-center gap-4 p-3 rounded-xl border transition-all text-left group/tool ${
                          formData.settings.toolAccess.includes(tool.id) 
                            ? 'bg-white/40 border-[#5381A5]/40' 
                            : 'bg-white/20 border-[#5381A5]/20 opacity-50 grayscale'
                        }`}
                      >
                        <span className={`material-symbols-outlined text-sm ${formData.settings.toolAccess.includes(tool.id) ? 'text-[#5381A5]' : 'text-[#163247]'}`}>
                          {tool.icon}
                        </span>
                        <div className="flex-1">
                          <div className="text-[11px] font-bold text-[#0b1b2b] uppercase tracking-tight">{tool.name}</div>
                        </div>
                        <span className="material-symbols-outlined text-lg text-[#5381A5]">
                          {formData.settings.toolAccess.includes(tool.id) ? 'check_circle' : 'circle'}
                        </span>
                      </button>
                    </HoverTooltip>
                  ))}
                </div>
              </div>
            </section>

            {/* Custom Branding */}
            <section className="bg-white/40 border border-[#5381A5]/30 rounded-2xl p-6 shadow-2xl">
              <div className="flex items-center gap-3 mb-6">
                <div className="p-2 bg-white/40 rounded-lg border border-[#5381A5]/20">
                  <span className="material-symbols-outlined text-[#5381A5] text-xl">palette</span>
                </div>
                <h2 className="text-[#0b1b2b] text-lg font-bold">Custom Branding</h2>
              </div>
              
              <div className="space-y-4">
                <div>
                  <label className="block text-[#163247] text-[10px] font-black uppercase tracking-widest mb-2">
                    Logo (SVG recommended)
                  </label>
                  <input
                    type="file"
                    accept=".svg,.png,.jpg,.jpeg"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) handleAssetUpload(file, 'logo');
                    }}
                    disabled={uploading === 'logo'}
                    className="w-full text-xs rounded-xl border border-[#5381A5]/30 bg-white/30 p-2 file:mr-4 file:py-1 file:px-3 file:rounded-lg file:border-0 file:text-xs file:font-semibold file:bg-[#5381A5] file:text-white hover:file:bg-[#437091] disabled:opacity-50"
                  />
                  {uploadStatus.logo && (
                    <p className={`text-[9px] mt-1 ${uploadStatus.logo.includes('Error') ? 'text-rose-600' : 'text-[#5381A5]'}`}>
                      {uploadStatus.logo}
                    </p>
                  )}
                </div>
                
                <div>
                  <label className="block text-[#163247] text-[10px] font-black uppercase tracking-widest mb-2">
                    Favicon (ICO)
                  </label>
                  <input
                    type="file"
                    accept=".ico"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) handleAssetUpload(file, 'favicon');
                    }}
                    disabled={uploading === 'favicon'}
                    className="w-full text-xs rounded-xl border border-[#5381A5]/30 bg-white/30 p-2 file:mr-4 file:py-1 file:px-3 file:rounded-lg file:border-0 file:text-xs file:font-semibold file:bg-[#5381A5] file:text-white hover:file:bg-[#437091] disabled:opacity-50"
                  />
                  {uploadStatus.favicon && (
                    <p className={`text-[9px] mt-1 ${uploadStatus.favicon.includes('Error') ? 'text-rose-600' : 'text-[#5381A5]'}`}>
                      {uploadStatus.favicon}
                    </p>
                  )}
                </div>
                
                <div>
                  <label className="block text-[#163247] text-[10px] font-black uppercase tracking-widest mb-2">
                    Favicon PNG (32x32)
                  </label>
                  <input
                    type="file"
                    accept=".png"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) handleAssetUpload(file, 'favicon-png');
                    }}
                    disabled={uploading === 'favicon-png'}
                    className="w-full text-xs rounded-xl border border-[#5381A5]/30 bg-white/30 p-2 file:mr-4 file:py-1 file:px-3 file:rounded-lg file:border-0 file:text-xs file:font-semibold file:bg-[#5381A5] file:text-white hover:file:bg-[#437091] disabled:opacity-50"
                  />
                  {uploadStatus['favicon-png'] && (
                    <p className={`text-[9px] mt-1 ${uploadStatus['favicon-png'].includes('Error') ? 'text-rose-600' : 'text-[#5381A5]'}`}>
                      {uploadStatus['favicon-png']}
                    </p>
                  )}
                </div>

                <HoverTooltip
                  title="Custom Branding"
                  description="Upload custom logo and favicon files. Logo appears in the sidebar, favicons appear in browser tabs. Changes take effect after page reload."
                >
                  <p className="text-[8px] text-[#163247] italic mt-2">
                    * Uploaded assets replace default branding. Reload page to see changes.
                  </p>
                </HoverTooltip>
              </div>
            </section>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SettingsView;
