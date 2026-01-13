import React, { useState, useEffect } from 'react';
import { uploadAsset, getCustomLogoUrl, getAssetUrl, checkAssetExists } from '../services/assetService';
import HoverTooltip from './HoverTooltip';
import { updateFaviconLinks } from '../utils/updateFavicon';
import { configureEmailTeams, setOAuthTokens } from '../services/emailTeamsService';
import { Twin, TwinSettings } from '../types';
import { INITIAL_TWINS } from '../constants';

interface OrchestratorSettingsProps {
  onClose: () => void;
}

interface OrchestratorSettingsData {
  userName: string;
  orchestratorUrl: string;
  gatewayUrl: string;
  memoryGrpcUrl: string;
  toolsGrpcUrl: string;
  openrouterApiKey: string;
  openrouterModel: string;
  qdrantUrl: string;
  customLogoUrl: string;
}

interface OrchestratorAgentSettings {
  temperature: number;
  topP: number;
  maxMemory: number;
  tokenLimit: number;
}

interface PromptCurrentResponse {
  prompt: string;
}

interface PromptUpdateResponse {
  success: boolean;
  message: string;
}

const OrchestratorSettings: React.FC<OrchestratorSettingsProps> = ({ onClose }) => {
  // Load settings from localStorage
  const loadSettings = (): OrchestratorSettingsData => {
    return {
      userName: localStorage.getItem('root_admin_user_name') || '',
      orchestratorUrl: localStorage.getItem('root_admin_orchestrator_url') || import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182',
      gatewayUrl: localStorage.getItem('root_admin_gateway_url') || import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181',
      memoryGrpcUrl: localStorage.getItem('root_admin_memory_grpc_url') || 'http://127.0.0.1:50052',
      toolsGrpcUrl: localStorage.getItem('root_admin_tools_grpc_url') || 'http://127.0.0.1:50054',
      openrouterApiKey: localStorage.getItem('root_admin_openrouter_api_key') || '',
      openrouterModel: localStorage.getItem('root_admin_openrouter_model') || 'anthropic/claude-3.5-sonnet',
      qdrantUrl: localStorage.getItem('root_admin_qdrant_url') || 'http://127.0.0.1:6334',
      customLogoUrl: getCustomLogoUrl(),
    };
  };

  const [settings, setSettings] = useState<OrchestratorSettingsData>(loadSettings);
  const [logoUrl, setLogoUrl] = useState(settings.customLogoUrl);
  const [uploading, setUploading] = useState(false);
  const [uploadStatus, setUploadStatus] = useState<string>('');
  const [hasChanges, setHasChanges] = useState(false);

  // --- Orchestrator Agent Settings (temperature, topP, etc.) ---
  const loadOrchestratorAgentSettings = (): OrchestratorAgentSettings => {
    try {
      const stored = localStorage.getItem('orchestrator_agent_settings');
      if (stored) {
        const parsed = JSON.parse(stored);
        return {
          temperature: parsed.temperature ?? 0.7,
          topP: parsed.topP ?? 0.9,
          maxMemory: parsed.maxMemory ?? 8,
          tokenLimit: parsed.tokenLimit ?? 64,
        };
      }
    } catch (e) {
      console.warn('[OrchestratorSettings] Failed to load agent settings:', e);
    }
    // Default to The Blue Flame's initial settings
    const orchestratorTwin = INITIAL_TWINS.find(t => t.id === 'twin-aegis');
    if (orchestratorTwin) {
      return {
        temperature: orchestratorTwin.settings.temperature,
        topP: orchestratorTwin.settings.topP,
        maxMemory: orchestratorTwin.settings.maxMemory,
        tokenLimit: orchestratorTwin.settings.tokenLimit,
      };
    }
    return {
      temperature: 0.7,
      topP: 0.9,
      maxMemory: 8,
      tokenLimit: 64,
    };
  };

  const [agentSettings, setAgentSettings] = useState<OrchestratorAgentSettings>(loadOrchestratorAgentSettings);
  const [agentSettingsChanged, setAgentSettingsChanged] = useState(false);

  // --- Orchestrator Persona (System Prompt Template) ---
  const [promptTemplate, setPromptTemplate] = useState<string>('');
  const [loadedPromptTemplate, setLoadedPromptTemplate] = useState<string>('');
  const [promptChangeSummary, setPromptChangeSummary] = useState<string>('');
  const [promptStatus, setPromptStatus] = useState<string>('');
  const [promptLoading, setPromptLoading] = useState<boolean>(false);

  // --- Email/Teams OAuth Configuration ---
  const [emailTeamsConfig, setEmailTeamsConfig] = useState({
    client_id: localStorage.getItem('email_teams_client_id') || '',
    client_secret: localStorage.getItem('email_teams_client_secret') || '',
    tenant_id: localStorage.getItem('email_teams_tenant_id') || '',
    user_email: localStorage.getItem('email_teams_user_email') || settings.userName || '',
    user_name: localStorage.getItem('email_teams_user_name') || settings.userName || '',
    redirect_uri: localStorage.getItem('email_teams_redirect_uri') || `${window.location.origin}/oauth/callback`,
  });
  const [oauthStatus, setOauthStatus] = useState<string>('');

  // Check for existing logo on mount
  useEffect(() => {
    const checkLogo = async () => {
      // Support multiple logo formats (backend may store png/jpg depending on upload)
      // Prefer raster formats first; older builds used to overwrite `custom-logo.svg`
      // even for PNG uploads, which can leave an invalid SVG file behind.
      const candidates = [
        'custom-logo.png',
        'custom-logo.jpg',
        'custom-logo.jpeg',
        'custom-logo.svg',
      ];
      for (const filename of candidates) {
        const url = getAssetUrl(filename);
        // eslint-disable-next-line no-await-in-loop
        const exists = await checkAssetExists(url);
        if (exists) {
          setLogoUrl(url);
          setSettings(prev => ({ ...prev, customLogoUrl: url }));
          return;
        }
      }
    };
    checkLogo();
  }, []);

  const fetchPromptCurrent = async () => {
    setPromptLoading(true);
    setPromptStatus('Loading current persona prompt...');
    try {
      const gatewayUrl = settings.gatewayUrl || import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
      const orchestratorUrl = settings.orchestratorUrl || import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

      let response = await fetch(`${gatewayUrl}/api/prompt/current`, {
        method: 'GET',
        headers: { Accept: 'application/json' },
      });

      // If gateway doesn't have the route, try orchestrator directly
      if (!response.ok && response.status === 404) {
        response = await fetch(`${orchestratorUrl}/v1/prompt/current`, {
          method: 'GET',
          headers: { Accept: 'application/json' },
        });
      }

      if (!response.ok) {
        throw new Error(`Failed to load persona prompt: ${response.status} ${response.statusText}`);
      }

      const data = (await response.json()) as PromptCurrentResponse;
      const prompt = typeof data?.prompt === 'string' ? data.prompt : '';
      setPromptTemplate(prompt);
      setLoadedPromptTemplate(prompt);
      setPromptStatus('Loaded current persona prompt.');
    } catch (e) {
      setPromptStatus(`Error: ${e instanceof Error ? e.message : 'Failed to load persona prompt'}`);
    } finally {
      setPromptLoading(false);
      setTimeout(() => setPromptStatus(''), 3000);
    }
  };

  const updatePromptTemplateRemote = async () => {
    if (!promptTemplate.trim()) {
      setPromptStatus('Error: Persona prompt cannot be empty.');
      return;
    }

    setPromptLoading(true);
    setPromptStatus('Applying persona prompt...');
    try {
      const gatewayUrl = settings.gatewayUrl || import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
      const orchestratorUrl = settings.orchestratorUrl || import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

      const body = JSON.stringify({
        new_prompt: promptTemplate,
        change_summary: promptChangeSummary,
      });

      let response = await fetch(`${gatewayUrl}/api/prompt/update`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      });

      // If gateway doesn't have the route, try orchestrator directly
      if (!response.ok && response.status === 404) {
        response = await fetch(`${orchestratorUrl}/v1/prompt/update`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body,
        });
      }

      if (!response.ok) {
        throw new Error(`Failed to apply persona prompt: ${response.status} ${response.statusText}`);
      }

      const data = (await response.json()) as PromptUpdateResponse;
      if (!data?.success) {
        throw new Error(data?.message || 'Prompt update failed');
      }

      setLoadedPromptTemplate(promptTemplate);
      setPromptChangeSummary('');
      setPromptStatus('Persona prompt applied successfully.');
    } catch (e) {
      setPromptStatus(`Error: ${e instanceof Error ? e.message : 'Failed to apply persona prompt'}`);
    } finally {
      setPromptLoading(false);
      setTimeout(() => setPromptStatus(''), 3000);
    }
  };

  const resetPromptTemplateRemote = async () => {
    if (!confirm('Reset Orchestrator persona prompt back to default? This will create a new history entry.')) {
      return;
    }

    setPromptLoading(true);
    setPromptStatus('Resetting persona prompt to default...');
    try {
      const gatewayUrl = settings.gatewayUrl || import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
      const orchestratorUrl = settings.orchestratorUrl || import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

      let response = await fetch(`${gatewayUrl}/api/prompt/reset`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });

      // If gateway doesn't have the route, try orchestrator directly
      if (!response.ok && response.status === 404) {
        response = await fetch(`${orchestratorUrl}/v1/prompt/reset`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({}),
        });
      }

      if (!response.ok) {
        throw new Error(`Failed to reset persona prompt: ${response.status} ${response.statusText}`);
      }

      // Reload prompt from server after reset.
      await fetchPromptCurrent();
      setPromptChangeSummary('');
      setPromptStatus('Persona prompt reset to default.');
    } catch (e) {
      setPromptStatus(`Error: ${e instanceof Error ? e.message : 'Failed to reset persona prompt'}`);
    } finally {
      setPromptLoading(false);
      setTimeout(() => setPromptStatus(''), 3000);
    }
  };

  const handleChange = (key: keyof OrchestratorSettingsData, value: string) => {
    setSettings(prev => ({ ...prev, [key]: value }));
    setHasChanges(true);
  };

  const handleSave = () => {
    // Save to localStorage
    const previousUserName = localStorage.getItem('root_admin_user_name');
    localStorage.setItem('root_admin_user_name', settings.userName);
    localStorage.setItem('root_admin_orchestrator_url', settings.orchestratorUrl);
    localStorage.setItem('root_admin_gateway_url', settings.gatewayUrl);
    localStorage.setItem('root_admin_memory_grpc_url', settings.memoryGrpcUrl);
    localStorage.setItem('root_admin_tools_grpc_url', settings.toolsGrpcUrl);
    localStorage.setItem('root_admin_openrouter_api_key', settings.openrouterApiKey);
    localStorage.setItem('root_admin_openrouter_model', settings.openrouterModel);
    localStorage.setItem('root_admin_qdrant_url', settings.qdrantUrl);

    // Save orchestrator agent settings
    if (agentSettingsChanged) {
      localStorage.setItem('orchestrator_agent_settings', JSON.stringify(agentSettings));
      // Trigger event to update orchestrator twin in App state
      window.dispatchEvent(new CustomEvent('orchestratorAgentSettingsChanged', { 
        detail: { settings: agentSettings } 
      }));
      setAgentSettingsChanged(false);
    }

    // Trigger custom event if user name changed (for immediate UI update)
    if (previousUserName !== settings.userName) {
      window.dispatchEvent(new CustomEvent('userNameChanged', { detail: { userName: settings.userName } }));
    }

    // Update environment variables (for current session)
    if (settings.orchestratorUrl) {
      (window as any).__ORCHESTRATOR_URL__ = settings.orchestratorUrl;
    }
    if (settings.gatewayUrl) {
      (window as any).__GATEWAY_URL__ = settings.gatewayUrl;
    }

    setHasChanges(false);
    alert('Settings saved successfully! Some changes may require a page refresh to take effect.');
  };

  const handleLogoUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    // Validate file type
    if (!file.type.startsWith('image/')) {
      setUploadStatus('Error: Please select an image file');
      return;
    }

    // Validate file size (max 5MB)
    if (file.size > 5 * 1024 * 1024) {
      setUploadStatus('Error: File size must be less than 5MB');
      return;
    }

    setUploading(true);
    setUploadStatus('Uploading...');

    try {
      // If a user uploads a logo, also generate & upload a matching 32x32 PNG favicon.
      const generateFaviconPngFromImageFile = async (srcFile: File): Promise<File> => {
        const objectUrl = URL.createObjectURL(srcFile);
        try {
          const img = new Image();
          img.decoding = 'async';
          // No crossOrigin needed for local blob URLs.
          await new Promise<void>((resolve, reject) => {
            img.onload = () => resolve();
            img.onerror = () => reject(new Error('Failed to decode logo image for favicon generation'));
            img.src = objectUrl;
          });

          const canvas = document.createElement('canvas');
          canvas.width = 32;
          canvas.height = 32;
          const ctx = canvas.getContext('2d');
          if (!ctx) {
            throw new Error('Canvas 2D context unavailable (cannot generate favicon)');
          }

          // Contain-fit into 32x32.
          ctx.clearRect(0, 0, 32, 32);
          const scale = Math.min(32 / img.width, 32 / img.height);
          const w = Math.max(1, Math.round(img.width * scale));
          const h = Math.max(1, Math.round(img.height * scale));
          const x = Math.floor((32 - w) / 2);
          const y = Math.floor((32 - h) / 2);
          ctx.drawImage(img, x, y, w, h);

          const blob: Blob = await new Promise((resolve, reject) => {
            canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('Failed to encode favicon PNG'))), 'image/png');
          });

          return new File([blob], 'custom-favicon-32.png', { type: 'image/png' });
        } finally {
          URL.revokeObjectURL(objectUrl);
        }
      };

      const result = await uploadAsset(file, 'logo');
      // backend returns `{ ok: true, asset_type, stored_path }`
      if (result.ok) {
        // Cache-bust so the newly uploaded asset shows immediately.
        // Also: use the returned stored_path to infer the filename/extension.
        const storedFilename = result.stored_path.split(/[/\\]/).pop() || 'custom-logo.svg';
        const newLogoUrl = `${getAssetUrl(storedFilename)}?v=${Date.now()}`;
        setLogoUrl(newLogoUrl);
        setSettings(prev => ({ ...prev, customLogoUrl: newLogoUrl }));
        setUploadStatus('Logo uploaded successfully!');
        setHasChanges(true);

        // Best-effort: generate + upload favicon PNG derived from the uploaded logo.
        // Failure here should not block the logo upload.
        try {
          const faviconFile = await generateFaviconPngFromImageFile(file);
          await uploadAsset(faviconFile, 'favicon-png');
          updateFaviconLinks();
        } catch (e) {
          console.warn('[OrchestratorSettings] Favicon generation/upload skipped:', e);
        }

        // Notify other components (e.g., SidebarLeft) to refresh logo immediately.
        window.dispatchEvent(new CustomEvent('logoChanged', { detail: { url: newLogoUrl } }));
      } else {
        setUploadStatus('Error: Upload failed');
      }
    } catch (error) {
      setUploadStatus(`Error: ${error instanceof Error ? error.message : 'Upload failed'}`);
    } finally {
      setUploading(false);
    }
  };

  const getUserDisplayName = () => {
    return settings.userName.trim() || 'FG_User';
  };

  // Load persona prompt on mount and whenever URL settings change.
  useEffect(() => {
    fetchPromptCurrent();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.gatewayUrl, settings.orchestratorUrl]);

  return (
    <div className="h-full flex flex-col bg-[#9EC9D9]">
      {/* Header */}
      <div className="border-b border-[#5381A5]/30 bg-[#90C3EA] px-6 py-4 flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-[#0b1b2b]">Orchestrator Settings</h1>
          <p className="text-xs text-[#163247] mt-1">Configure system-wide settings and preferences</p>
        </div>
        <div className="flex items-center gap-3">
          {(hasChanges || agentSettingsChanged) && (
            <span className="text-xs text-amber-600 font-bold uppercase tracking-wider">Unsaved Changes</span>
          )}
          <button
            onClick={handleSave}
            disabled={!hasChanges && !agentSettingsChanged}
            className={`px-4 py-2 rounded-lg text-sm font-bold transition-all ${
              hasChanges
                ? 'bg-[#5381A5] text-white hover:bg-[#3d6a8a]'
                : 'bg-gray-300 text-gray-500 cursor-not-allowed'
            }`}
          >
            Save Settings
          </button>
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-sm font-bold bg-[#78A2C2] text-[#0b1b2b] hover:bg-[#5381A5] hover:text-white transition-all"
          >
            Close
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto space-y-6">
          {/* USER SETTINGS */}
          <section className="bg-white/60 border border-[#5381A5]/30 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[#5381A5]">person</span>
              <h2 className="text-lg font-bold text-[#0b1b2b]">User Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  User Name
                </label>
                <HoverTooltip
                  title="User Name"
                  description="Enter your name so The Blue Flame orchestrator can address you personally. If left empty, it will use 'FG_User' as the default."
                >
                  <input
                    type="text"
                    value={settings.userName}
                    onChange={(e) => handleChange('userName', e.target.value)}
                    placeholder="FG_User"
                    className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                  />
                </HoverTooltip>
                <p className="text-xs text-[#163247] mt-2">
                  Current display name: <span className="font-bold">{getUserDisplayName()}</span>
                </p>
              </div>
            </div>
          </section>

          {/* ORCHESTRATOR SETTINGS */}
          <section className="bg-white/60 border border-[#5381A5]/30 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[#5381A5]">settings</span>
              <h2 className="text-lg font-bold text-[#0b1b2b]">Orchestrator Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Orchestrator URL
                </label>
                <input
                  type="text"
                  value={settings.orchestratorUrl}
                  onChange={(e) => handleChange('orchestratorUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Gateway URL
                </label>
                <input
                  type="text"
                  value={settings.gatewayUrl}
                  onChange={(e) => handleChange('gatewayUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
            </div>
          </section>

          {/* ORCHESTRATOR AGENT SETTINGS */}
          <section className="bg-white/60 border border-[#5381A5]/30 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[#5381A5]">tune</span>
              <h2 className="text-lg font-bold text-[#0b1b2b]">Agent Settings (The Blue Flame)</h2>
            </div>
            <div className="space-y-5">
              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Logic Variance (Temperature)</span>
                  <span className="text-xs font-mono text-[#5381A5] font-bold">{agentSettings.temperature.toFixed(2)}</span>
                </div>
                <HoverTooltip
                  title="Logic Variance (Temperature)"
                  description="Higher values increase creativity/variance; lower values make the agent more deterministic and policy-following."
                >
                  <input 
                    type="range" min="0" max="1.5" step="0.05"
                    value={agentSettings.temperature}
                    onChange={(e) => {
                      setAgentSettings(prev => ({ ...prev, temperature: parseFloat(e.target.value) }));
                      setAgentSettingsChanged(true);
                    }}
                    className="w-full h-1.5 bg-white/40 rounded-lg appearance-none cursor-pointer accent-[#5381A5]"
                  />
                </HoverTooltip>
              </div>

              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Top P (Nucleus Sampling)</span>
                  <span className="text-xs font-mono text-[#5381A5] font-bold">{agentSettings.topP.toFixed(2)}</span>
                </div>
                <HoverTooltip
                  title="Top P (Nucleus Sampling)"
                  description="Controls diversity via nucleus sampling. Lower values (0.1-0.5) = more focused, higher values (0.9-1.0) = more diverse outputs."
                >
                  <input 
                    type="range" min="0.1" max="1.0" step="0.05"
                    value={agentSettings.topP}
                    onChange={(e) => {
                      setAgentSettings(prev => ({ ...prev, topP: parseFloat(e.target.value) }));
                      setAgentSettingsChanged(true);
                    }}
                    className="w-full h-1.5 bg-white/40 rounded-lg appearance-none cursor-pointer accent-[#5381A5]"
                  />
                </HoverTooltip>
              </div>

              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Context Shards (Token Limit)</span>
                  <span className="text-xs font-mono text-[#5381A5] font-bold">{agentSettings.tokenLimit}K</span>
                </div>
                <HoverTooltip
                  title="Context Shards (Token Limit)"
                  description="Maximum context window size used for planning. Higher limits improve recall but may increase latency/cost."
                >
                  <input 
                    type="range" min="16" max="128" step="16"
                    value={agentSettings.tokenLimit}
                    onChange={(e) => {
                      setAgentSettings(prev => ({ ...prev, tokenLimit: parseInt(e.target.value) }));
                      setAgentSettingsChanged(true);
                    }}
                    className="w-full h-1.5 bg-white/40 rounded-lg appearance-none cursor-pointer accent-[#5381A5]"
                  />
                </HoverTooltip>
              </div>

              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[#163247] text-[10px] font-black uppercase tracking-widest">Memory Capacity</span>
                  <span className="text-xs font-mono text-[#5381A5] font-bold">{agentSettings.maxMemory}GB</span>
                </div>
                <HoverTooltip
                  title="Memory Capacity (Max Memory)"
                  description="Maximum memory allocation for this agent's operations. Higher values allow more complex reasoning but consume more resources."
                >
                  <input 
                    type="range" min="1" max="32" step="1"
                    value={agentSettings.maxMemory}
                    onChange={(e) => {
                      setAgentSettings(prev => ({ ...prev, maxMemory: parseInt(e.target.value) }));
                      setAgentSettingsChanged(true);
                    }}
                    className="w-full h-1.5 bg-white/40 rounded-lg appearance-none cursor-pointer accent-[#5381A5]"
                  />
                </HoverTooltip>
              </div>

              {agentSettingsChanged && (
                <div className="pt-2 border-t border-[#5381A5]/20">
                  <p className="text-xs text-amber-700 font-bold uppercase tracking-wider">
                    Agent settings have been modified. Click "Save Settings" to apply.
                  </p>
                </div>
              )}
            </div>
          </section>

          {/* ORCHESTRATOR AGENT / PERSONA */}
          <section className="bg-white/60 border border-[#5381A5]/30 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[#5381A5]">psychology</span>
              <h2 className="text-lg font-bold text-[#0b1b2b]">Orchestrator Agent / Persona</h2>
            </div>

            <div className="space-y-4">
              <div className="text-xs text-[#163247]">
                This is the Orchestrator system prompt template. Supported placeholders:
                <span className="font-mono"> {'{twin_id}'} </span>
                and
                <span className="font-mono"> {'{user_name}'} </span>.
              </div>

              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Persona Prompt Template
                </label>
                <textarea
                  value={promptTemplate}
                  onChange={(e) => setPromptTemplate(e.target.value)}
                  rows={12}
                  className="w-full px-4 py-3 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5] font-mono text-xs"
                  placeholder="System prompt template..."
                />
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onClick={fetchPromptCurrent}
                    disabled={promptLoading}
                    className="px-3 py-2 rounded-lg text-xs font-bold bg-[#78A2C2] text-[#0b1b2b] hover:bg-[#5381A5] hover:text-white transition-all disabled:opacity-50"
                  >
                    Reload From Server
                  </button>
                  <button
                    type="button"
                    onClick={resetPromptTemplateRemote}
                    disabled={promptLoading}
                    className="px-3 py-2 rounded-lg text-xs font-bold bg-white/80 border border-[#5381A5]/30 text-[#163247] hover:bg-white transition-all disabled:opacity-50"
                  >
                    Reset To Default
                  </button>
                </div>
              </div>

              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Change Summary (optional)
                </label>
                <input
                  type="text"
                  value={promptChangeSummary}
                  onChange={(e) => setPromptChangeSummary(e.target.value)}
                  placeholder="e.g., tighten tool authorization language"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>

              <div className="flex items-center gap-3">
                <button
                  type="button"
                  onClick={updatePromptTemplateRemote}
                  disabled={promptLoading || promptTemplate.trim() === loadedPromptTemplate.trim()}
                  className={`px-4 py-2 rounded-lg text-sm font-bold transition-all ${
                    promptLoading || promptTemplate.trim() === loadedPromptTemplate.trim()
                      ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                      : 'bg-[#5381A5] text-white hover:bg-[#3d6a8a]'
                  }`}
                >
                  Apply Persona
                </button>
                {promptTemplate.trim() !== loadedPromptTemplate.trim() && (
                  <span className="text-xs text-amber-700 font-bold uppercase tracking-wider">
                    Unsaved Persona Changes
                  </span>
                )}
              </div>

              {promptStatus && (
                <p className={`text-xs ${promptStatus.startsWith('Error:') ? 'text-rose-600' : 'text-emerald-700'}`}>
                  {promptStatus}
                </p>
              )}
            </div>
          </section>

          {/* API SETTINGS */}
          <section className="bg-white/60 border border-[#5381A5]/30 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[#5381A5]">api</span>
              <h2 className="text-lg font-bold text-[#0b1b2b]">API Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  OpenRouter API Key
                </label>
                <input
                  type="password"
                  value={settings.openrouterApiKey}
                  onChange={(e) => handleChange('openrouterApiKey', e.target.value)}
                  placeholder="sk-or-..."
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  OpenRouter Model
                </label>
                <input
                  type="text"
                  value={settings.openrouterModel}
                  onChange={(e) => handleChange('openrouterModel', e.target.value)}
                  placeholder="anthropic/claude-3.5-sonnet"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Memory Service gRPC URL
                </label>
                <input
                  type="text"
                  value={settings.memoryGrpcUrl}
                  onChange={(e) => handleChange('memoryGrpcUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Tools Service gRPC URL
                </label>
                <input
                  type="text"
                  value={settings.toolsGrpcUrl}
                  onChange={(e) => handleChange('toolsGrpcUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Qdrant URL
                </label>
                <input
                  type="text"
                  value={settings.qdrantUrl}
                  onChange={(e) => handleChange('qdrantUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
            </div>
          </section>

          {/* EMAIL/TEAMS MONITORING */}
          <section className="bg-white/60 border border-[#5381A5]/30 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[#5381A5]">mail</span>
              <h2 className="text-lg font-bold text-[#0b1b2b]">Email & Teams Monitoring</h2>
            </div>
            <div className="space-y-4">
              <div className="text-xs text-[#163247] bg-[#90C3EA]/30 p-3 rounded-lg">
                <p className="font-bold mb-1">Microsoft Graph API OAuth Setup:</p>
                <ol className="list-decimal list-inside space-y-1 ml-2">
                  <li>Register an app in Azure Portal (portal.azure.com)</li>
                  <li>Add API permissions: Mail.Read, Mail.Send, Chat.Read, Chat.Send, User.Read</li>
                  <li>Create a client secret and copy the values below</li>
                  <li>Set redirect URI to: <code className="font-mono bg-white/60 px-1 rounded">{emailTeamsConfig.redirect_uri}</code></li>
                  <li>After configuring, complete OAuth flow to get access tokens</li>
                </ol>
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Azure Client ID
                </label>
                <input
                  type="text"
                  value={emailTeamsConfig.client_id}
                  onChange={(e) => {
                    setEmailTeamsConfig({ ...emailTeamsConfig, client_id: e.target.value });
                    localStorage.setItem('email_teams_client_id', e.target.value);
                  }}
                  placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5] font-mono text-xs"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Azure Client Secret
                </label>
                <input
                  type="password"
                  value={emailTeamsConfig.client_secret}
                  onChange={(e) => {
                    setEmailTeamsConfig({ ...emailTeamsConfig, client_secret: e.target.value });
                    localStorage.setItem('email_teams_client_secret', e.target.value);
                  }}
                  placeholder="Enter client secret"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5] font-mono text-xs"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Azure Tenant ID
                </label>
                <input
                  type="text"
                  value={emailTeamsConfig.tenant_id}
                  onChange={(e) => {
                    setEmailTeamsConfig({ ...emailTeamsConfig, tenant_id: e.target.value });
                    localStorage.setItem('email_teams_tenant_id', e.target.value);
                  }}
                  placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5] font-mono text-xs"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  User Email
                </label>
                <input
                  type="email"
                  value={emailTeamsConfig.user_email}
                  onChange={(e) => {
                    setEmailTeamsConfig({ ...emailTeamsConfig, user_email: e.target.value });
                    localStorage.setItem('email_teams_user_email', e.target.value);
                  }}
                  placeholder="user@example.com"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  User Name
                </label>
                <input
                  type="text"
                  value={emailTeamsConfig.user_name}
                  onChange={(e) => {
                    setEmailTeamsConfig({ ...emailTeamsConfig, user_name: e.target.value });
                    localStorage.setItem('email_teams_user_name', e.target.value);
                  }}
                  placeholder="Your Name"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Redirect URI
                </label>
                <input
                  type="text"
                  value={emailTeamsConfig.redirect_uri}
                  onChange={(e) => {
                    setEmailTeamsConfig({ ...emailTeamsConfig, redirect_uri: e.target.value });
                    localStorage.setItem('email_teams_redirect_uri', e.target.value);
                  }}
                  placeholder="http://localhost:5173/oauth/callback"
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5] font-mono text-xs"
                />
              </div>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={async () => {
                    try {
                      setOauthStatus('Configuring...');
                      await configureEmailTeams(emailTeamsConfig);
                      setOauthStatus('Configuration saved! Complete OAuth flow to activate.');
                    } catch (error) {
                      setOauthStatus(`Error: ${error instanceof Error ? error.message : 'Unknown error'}`);
                    }
                  }}
                  className="px-4 py-2 rounded-lg text-sm font-bold bg-[#5381A5] text-white hover:bg-[#3d6a8a] transition-all"
                >
                  Save Configuration
                </button>
                <button
                  type="button"
                  onClick={() => {
                    // Generate OAuth URL for Microsoft
                    const redirectUri = emailTeamsConfig.redirect_uri || `${window.location.origin}/oauth/callback`;
                    const authUrl = `https://login.microsoftonline.com/${emailTeamsConfig.tenant_id}/oauth2/v2.0/authorize?client_id=${emailTeamsConfig.client_id}&response_type=code&redirect_uri=${encodeURIComponent(redirectUri)}&response_mode=query&scope=Mail.Read Mail.Send Chat.Read Chat.Send User.Read offline_access`;
                    
                    // Open in popup window
                    const popup = window.open(
                      authUrl,
                      'oauth',
                      'width=600,height=700,scrollbars=yes,resizable=yes'
                    );
                    
                    // Listen for OAuth success message from popup
                    const messageHandler = (event: MessageEvent) => {
                      if (event.data.type === 'oauth_success') {
                        setOauthStatus('OAuth authentication successful! Email/Teams monitoring is now active.');
                        window.removeEventListener('message', messageHandler);
                        if (popup) popup.close();
                      } else if (event.data.type === 'oauth_error') {
                        setOauthStatus(`OAuth error: ${event.data.error}`);
                        window.removeEventListener('message', messageHandler);
                        if (popup) popup.close();
                      }
                    };
                    
                    window.addEventListener('message', messageHandler);
                    
                    // Check if popup was closed manually
                    const checkClosed = setInterval(() => {
                      if (popup?.closed) {
                        clearInterval(checkClosed);
                        window.removeEventListener('message', messageHandler);
                      }
                    }, 1000);
                    
                    setOauthStatus('OAuth window opened. Please complete authentication in the popup window.');
                  }}
                  disabled={!emailTeamsConfig.client_id || !emailTeamsConfig.tenant_id}
                  className="px-4 py-2 rounded-lg text-sm font-bold bg-[#78A2C2] text-[#0b1b2b] hover:bg-[#5381A5] hover:text-white transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Start OAuth Flow
                </button>
              </div>
              {oauthStatus && (
                <p className={`text-xs ${oauthStatus.startsWith('Error') ? 'text-rose-600' : 'text-emerald-600'}`}>
                  {oauthStatus}
                </p>
              )}
              <div className="mt-4 p-3 bg-[#90C3EA]/30 rounded-lg">
                <p className="text-xs text-[#163247] mb-2">
                  <strong>Note:</strong> The OAuth flow will automatically exchange the authorization code for tokens.
                  You don't need to manually paste tokens anymore. If the automatic flow fails, you can still manually set tokens below.
                </p>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Manual Token Entry (Fallback)
                </label>
                <textarea
                  placeholder="Only use this if automatic OAuth flow fails. Paste access token here."
                  onChange={async (e) => {
                    const token = e.target.value.trim();
                    if (token) {
                      try {
                        await setOAuthTokens(token);
                        setOauthStatus('Access token set successfully! Email/Teams monitoring is now active.');
                      } catch (error) {
                        setOauthStatus(`Error setting token: ${error instanceof Error ? error.message : 'Unknown error'}`);
                      }
                    }
                  }}
                  className="w-full px-4 py-2 rounded-lg border border-[#5381A5]/30 bg-white/80 text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5] font-mono text-xs"
                  rows={3}
                />
              </div>
            </div>
          </section>

          {/* LOGO SETTINGS */}
          <section className="bg-white/60 border border-[#5381A5]/30 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[#5381A5]">image</span>
              <h2 className="text-lg font-bold text-[#0b1b2b]">Logo Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[#163247] mb-2 uppercase tracking-wider">
                  Custom Logo
                </label>
                <div className="flex items-center gap-4">
                  <div className="w-24 h-24 rounded-lg border-2 border-[#5381A5]/30 bg-white/80 flex items-center justify-center overflow-hidden">
                    {logoUrl ? (
                      <img
                        src={logoUrl}
                        alt="Custom Logo"
                        className="w-full h-full object-contain"
                        onError={() => setLogoUrl('/ferrellgas-agi-badge.svg')}
                      />
                    ) : (
                      <span className="material-symbols-outlined text-[#5381A5]">image</span>
                    )}
                  </div>
                  <div className="flex-1">
                    <input
                      type="file"
                      accept="image/*"
                      onChange={handleLogoUpload}
                      disabled={uploading}
                      className="hidden"
                      id="logo-upload"
                    />
                    <label
                      htmlFor="logo-upload"
                      className={`inline-block px-4 py-2 rounded-lg text-sm font-bold cursor-pointer transition-all ${
                        uploading
                          ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                          : 'bg-[#5381A5] text-white hover:bg-[#3d6a8a]'
                      }`}
                    >
                      {uploading ? 'Uploading...' : 'Upload Logo'}
                    </label>
                    {uploadStatus && (
                      <p className={`text-xs mt-2 ${uploadStatus.startsWith('Error') ? 'text-rose-600' : 'text-emerald-600'}`}>
                        {uploadStatus}
                      </p>
                    )}
                    <p className="text-xs text-[#163247] mt-2">
                      Supported formats: PNG, JPG, SVG. Max size: 5MB
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
};

export default OrchestratorSettings;
