import React, { useState, useEffect } from 'react';
import { uploadAsset, getCustomLogoUrl, getAssetUrl, checkAssetExists } from '../services/assetService';
import HoverTooltip from './HoverTooltip';
import { updateFaviconLinks } from '../utils/updateFavicon';
import { configureEmailTeams, setOAuthTokens } from '../services/emailTeamsService';
import { readEnvFile, updateEnvFile, SECURITY_GATES, isSecurityGateEnabled, setSecurityGateValue, type EnvReadResponse } from '../services/envService';
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

// --- Personalization Preferences (per twin_id/user_id) ---
interface PersonaPreset {
  id: string;
  label: string;
  description: string;
  overlay: string;
}

type Verbosity = 'minimal' | 'balanced' | 'detailed';

interface UserProfile {
  nickname: string;
  occupation: string;
  about: string;
}

interface UserPreferences {
  profile: UserProfile;
  persona_preset: string;
  custom_instructions: string;
  verbosity: Verbosity;
  enable_cynical: boolean;
  enable_sarcastic: boolean;
  updated_at?: string;
}

interface PreferencesPresetsResponse {
  presets: PersonaPreset[];
}

interface PreferencesUpdateResponse {
  success: boolean;
  message: string;
  preferences?: UserPreferences;
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
    // Default to Phoenix's initial settings
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

  // --- Personalization Preferences (Style & Tone) ---
  const getDefaultPreferencesTwinId = () => {
    // This app uses `pagi_user_id` as the Orchestrator `twin_id` for the operator.
    // Allow overriding it for deterministic testing.
    return (
      localStorage.getItem('root_admin_preferences_twin_id') ||
      localStorage.getItem('pagi_user_id') ||
      (import.meta as any).env?.VITE_FORCE_TWIN_ID ||
      'twin-aegis'
    );
  };

  const [preferencesTwinId, setPreferencesTwinId] = useState<string>(getDefaultPreferencesTwinId());
  const [personaPresets, setPersonaPresets] = useState<PersonaPreset[]>([]);
  const [prefs, setPrefs] = useState<UserPreferences>({
    profile: { nickname: '', occupation: '', about: '' },
    persona_preset: 'default',
    custom_instructions: '',
    verbosity: 'balanced',
    enable_cynical: false,
    enable_sarcastic: false,
    updated_at: undefined,
  });
  const [loadedPrefs, setLoadedPrefs] = useState<UserPreferences | null>(null);
  const [prefsStatus, setPrefsStatus] = useState<string>('');
  const [prefsLoading, setPrefsLoading] = useState<boolean>(false);
  const [prefsDirty, setPrefsDirty] = useState<boolean>(false);

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

  // --- Environment Variables / Security Gates ---
  const [envVars, setEnvVars] = useState<Record<string, string>>({});
  const [envFilePath, setEnvFilePath] = useState<string>('');
  const [envLoading, setEnvLoading] = useState<boolean>(false);
  const [envStatus, setEnvStatus] = useState<string>('');
  const [envHasChanges, setEnvHasChanges] = useState<boolean>(false);

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
      if (prompt) {
        setPromptTemplate(prompt);
        setLoadedPromptTemplate(prompt);
        setPromptStatus('Loaded current persona prompt.');
      } else {
        setPromptStatus('Warning: System prompt is empty. Using default.');
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : 'Failed to load persona prompt';
      setPromptStatus(`Error: ${errorMsg}. The field below shows the current system prompt when available.`);
      // Don't clear the existing prompt if there's an error - keep what was loaded before
      console.warn('[OrchestratorSettings] Failed to fetch prompt:', e);
    } finally {
      setPromptLoading(false);
      setTimeout(() => setPromptStatus(''), 5000);
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

  const fetchPersonaPresets = async () => {
    setPrefsLoading(true);
    setPrefsStatus('Loading persona presets...');
    try {
      const gatewayUrl = settings.gatewayUrl || import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
      const orchestratorUrl = settings.orchestratorUrl || import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

      let response = await fetch(`${gatewayUrl}/api/preferences/presets`, {
        method: 'GET',
        headers: { Accept: 'application/json' },
      });
      if (!response.ok && response.status === 404) {
        response = await fetch(`${orchestratorUrl}/v1/preferences/presets`, {
          method: 'GET',
          headers: { Accept: 'application/json' },
        });
      }
      if (!response.ok) {
        throw new Error(`Failed to load persona presets: ${response.status} ${response.statusText}`);
      }

      const data = (await response.json()) as PreferencesPresetsResponse;
      setPersonaPresets(Array.isArray(data?.presets) ? data.presets : []);
      setPrefsStatus('Loaded persona presets.');
    } catch (e) {
      setPrefsStatus(`Error: ${e instanceof Error ? e.message : 'Failed to load persona presets'}`);
    } finally {
      setPrefsLoading(false);
      setTimeout(() => setPrefsStatus(''), 4000);
    }
  };

  const fetchPreferencesForTwin = async (twinId: string) => {
    const tid = twinId.trim();
    if (!tid) {
      setPrefsStatus('Error: preferences twin_id cannot be empty.');
      return;
    }

    setPrefsLoading(true);
    setPrefsStatus('Loading personalization preferences...');
    try {
      const gatewayUrl = settings.gatewayUrl || import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
      const orchestratorUrl = settings.orchestratorUrl || import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

      const qs = new URLSearchParams({ twin_id: tid });
      let response = await fetch(`${gatewayUrl}/api/preferences/get?${qs.toString()}`, {
        method: 'GET',
        headers: { Accept: 'application/json' },
      });
      if (!response.ok && response.status === 404) {
        response = await fetch(`${orchestratorUrl}/v1/preferences/get?${qs.toString()}`, {
          method: 'GET',
          headers: { Accept: 'application/json' },
        });
      }

      if (!response.ok) {
        throw new Error(`Failed to load preferences: ${response.status} ${response.statusText}`);
      }

      const data = (await response.json()) as UserPreferences;
      const normalized: UserPreferences = {
        profile: {
          nickname: data?.profile?.nickname ?? '',
          occupation: data?.profile?.occupation ?? '',
          about: data?.profile?.about ?? '',
        },
        persona_preset: (data?.persona_preset ?? 'default') as string,
        custom_instructions: data?.custom_instructions ?? '',
        verbosity: ((data as any)?.verbosity ?? 'balanced') as Verbosity,
        enable_cynical: !!(data as any)?.enable_cynical,
        enable_sarcastic: !!(data as any)?.enable_sarcastic,
        updated_at: (data as any)?.updated_at,
      };

      setPrefs(normalized);
      setLoadedPrefs(normalized);
      setPrefsDirty(false);
      setPrefsStatus('Loaded personalization preferences.');
    } catch (e) {
      setPrefsStatus(`Error: ${e instanceof Error ? e.message : 'Failed to load preferences'}`);
    } finally {
      setPrefsLoading(false);
      setTimeout(() => setPrefsStatus(''), 5000);
    }
  };

  const savePreferences = async () => {
    const tid = preferencesTwinId.trim();
    if (!tid) {
      setPrefsStatus('Error: preferences twin_id cannot be empty.');
      return;
    }

    setPrefsLoading(true);
    setPrefsStatus('Saving personalization preferences...');
    try {
      const gatewayUrl = settings.gatewayUrl || import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';
      const orchestratorUrl = settings.orchestratorUrl || import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

      const body = JSON.stringify({
        twin_id: tid,
        profile: prefs.profile,
        persona_preset: prefs.persona_preset,
        custom_instructions: prefs.custom_instructions,
        verbosity: prefs.verbosity,
        enable_cynical: prefs.enable_cynical,
        enable_sarcastic: prefs.enable_sarcastic,
      });

      let response = await fetch(`${gatewayUrl}/api/preferences/update`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      });
      if (!response.ok && response.status === 404) {
        response = await fetch(`${orchestratorUrl}/v1/preferences/update`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body,
        });
      }

      if (!response.ok) {
        throw new Error(`Failed to save preferences: ${response.status} ${response.statusText}`);
      }

      const data = (await response.json()) as PreferencesUpdateResponse;
      if (!data?.success) {
        throw new Error(data?.message || 'Preferences update failed');
      }

      if (data.preferences) {
        setPrefs(data.preferences);
        setLoadedPrefs(data.preferences);
      } else {
        // Reload as a fallback.
        await fetchPreferencesForTwin(tid);
      }
      localStorage.setItem('root_admin_preferences_twin_id', tid);
      setPrefsDirty(false);
      setPrefsStatus('Personalization preferences saved.');
    } catch (e) {
      setPrefsStatus(`Error: ${e instanceof Error ? e.message : 'Failed to save preferences'}`);
    } finally {
      setPrefsLoading(false);
      setTimeout(() => setPrefsStatus(''), 5000);
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

  // Load personalization presets and current preferences.
  useEffect(() => {
    fetchPersonaPresets();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.gatewayUrl, settings.orchestratorUrl]);

  useEffect(() => {
    fetchPreferencesForTwin(preferencesTwinId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preferencesTwinId, settings.gatewayUrl, settings.orchestratorUrl]);

  // Load environment variables on mount
  const fetchEnvFile = async () => {
    setEnvLoading(true);
    setEnvStatus('Loading environment variables...');
    try {
      const data = await readEnvFile();
      setEnvVars(data.env_vars);
      setEnvFilePath(data.env_file_path);
      setEnvStatus('Environment variables loaded.');
      setEnvHasChanges(false);
    } catch (e) {
      setEnvStatus(`Error: ${e instanceof Error ? e.message : 'Failed to load environment variables'}`);
    } finally {
      setEnvLoading(false);
      setTimeout(() => setEnvStatus(''), 3000);
    }
  };

  useEffect(() => {
    fetchEnvFile();
  }, [settings.gatewayUrl, settings.orchestratorUrl]);

  const handleSecurityGateToggle = (gateKey: string, enabled: boolean) => {
    const newEnvVars = { ...envVars };
    newEnvVars[gateKey] = setSecurityGateValue(enabled);
    setEnvVars(newEnvVars);
    setEnvHasChanges(true);
  };

  const handleSaveEnvFile = async () => {
    setEnvLoading(true);
    setEnvStatus('Saving environment variables...');
    try {
      const result = await updateEnvFile(envVars);
      if (result.success) {
        setEnvStatus('Environment variables saved successfully. Services may need to be restarted for changes to take effect.');
        setEnvHasChanges(false);
        // Reload to get updated values
        await fetchEnvFile();
      } else {
        setEnvStatus(`Error: ${result.message}`);
      }
    } catch (e) {
      setEnvStatus(`Error: ${e instanceof Error ? e.message : 'Failed to save environment variables'}`);
    } finally {
      setEnvLoading(false);
      setTimeout(() => setEnvStatus(''), 5000);
    }
  };

  return (
    <div className="h-full flex flex-col bg-[var(--bg-primary)]">
      {/* Header */}
      <div className="border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)] px-6 py-4 flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-[var(--text-primary)]">Orchestrator Settings</h1>
          <p className="text-xs text-[var(--text-secondary)] mt-1">Configure system-wide settings and preferences</p>
        </div>
        <div className="flex items-center gap-3">
          {(hasChanges || agentSettingsChanged) && (
            <span className="text-xs text-[var(--warning)] font-bold uppercase tracking-wider">Unsaved Changes</span>
          )}
          <button
            onClick={handleSave}
            disabled={!hasChanges && !agentSettingsChanged}
            className={`px-4 py-2 rounded-lg text-sm font-bold transition-all ${
              hasChanges
                ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)]'
                : 'bg-[rgb(var(--surface-rgb)/0.35)] text-[var(--text-muted)] cursor-not-allowed'
            }`}
          >
            Save Settings
          </button>
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-sm font-bold bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[var(--bg-steel)] hover:text-[var(--text-on-accent)] transition-all"
          >
            Close
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto space-y-6">
          {/* USER SETTINGS */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">person</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">User Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  User Name
                </label>
                  <HoverTooltip
                    title="User Name"
                    description="Enter your name so Phoenix (Ferrellgas Blue Flame) can address you personally. If left empty, it will use 'FG_User' as the default."
                  >
                  <input
                    type="text"
                    value={settings.userName}
                    onChange={(e) => handleChange('userName', e.target.value)}
                    placeholder="FG_User"
                    className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                  />
                </HoverTooltip>
                <p className="text-xs text-[var(--text-secondary)] mt-2">
                  Current display name: <span className="font-bold">{getUserDisplayName()}</span>
                </p>
              </div>
            </div>
          </section>

          {/* ORCHESTRATOR SETTINGS */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">settings</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">Orchestrator Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Orchestrator URL
                </label>
                <input
                  type="text"
                  value={settings.orchestratorUrl}
                  onChange={(e) => handleChange('orchestratorUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Gateway URL
                </label>
                <input
                  type="text"
                  value={settings.gatewayUrl}
                  onChange={(e) => handleChange('gatewayUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
            </div>
          </section>

          {/* ORCHESTRATOR AGENT SETTINGS */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">tune</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">Agent Settings (Phoenix)</h2>
            </div>
            <div className="space-y-5">
              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[var(--text-secondary)] text-[10px] font-black uppercase tracking-widest">Logic Variance (Temperature)</span>
                  <span className="text-xs font-mono text-[var(--bg-steel)] font-bold">{agentSettings.temperature.toFixed(2)}</span>
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
                    className="w-full h-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg appearance-none cursor-pointer accent-[var(--bg-steel)]"
                  />
                </HoverTooltip>
              </div>

              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[var(--text-secondary)] text-[10px] font-black uppercase tracking-widest">Top P (Nucleus Sampling)</span>
                  <span className="text-xs font-mono text-[var(--bg-steel)] font-bold">{agentSettings.topP.toFixed(2)}</span>
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
                    className="w-full h-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg appearance-none cursor-pointer accent-[var(--bg-steel)]"
                  />
                </HoverTooltip>
              </div>

              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[var(--text-secondary)] text-[10px] font-black uppercase tracking-widest">Context Shards (Token Limit)</span>
                  <span className="text-xs font-mono text-[var(--bg-steel)] font-bold">{agentSettings.tokenLimit}K</span>
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
                    className="w-full h-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg appearance-none cursor-pointer accent-[var(--bg-steel)]"
                  />
                </HoverTooltip>
              </div>

              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-[var(--text-secondary)] text-[10px] font-black uppercase tracking-widest">Memory Capacity</span>
                  <span className="text-xs font-mono text-[var(--bg-steel)] font-bold">{agentSettings.maxMemory}GB</span>
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
                    className="w-full h-1.5 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg appearance-none cursor-pointer accent-[var(--bg-steel)]"
                  />
                </HoverTooltip>
              </div>

              {agentSettingsChanged && (
                <div className="pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                  <p className="text-xs text-[rgb(var(--warning-rgb)/0.9)] font-bold uppercase tracking-wider">
                    Agent settings have been modified. Click "Save Settings" to apply.
                  </p>
                </div>
              )}
            </div>
          </section>

          {/* ORCHESTRATOR AGENT / PERSONA */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">psychology</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">Orchestrator Agent / Persona</h2>
            </div>

            <div className="space-y-4">
              <div className="text-xs text-[var(--text-secondary)]">
                This field displays the current Orchestrator system prompt template. Supported placeholders:
                <span className="font-mono"> {'{twin_id}'} </span>
                and
                <span className="font-mono"> {'{user_name}'} </span>.
                {promptTemplate && (
                  <span className="block mt-1 text-[rgb(var(--success-rgb)/0.9)] font-bold">
                    ✓ Current system prompt loaded ({promptTemplate.length} characters)
                  </span>
                )}
              </div>

              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Persona Prompt Template (Current System Prompt)
                </label>
                <textarea
                  value={promptTemplate}
                  onChange={(e) => setPromptTemplate(e.target.value)}
                  rows={12}
                  className="w-full px-4 py-3 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
                  placeholder={promptLoading ? "Loading current system prompt..." : "System prompt template... (Click 'Reload From Server' if empty)"}
                />
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onClick={fetchPromptCurrent}
                    disabled={promptLoading}
                    className="px-3 py-2 rounded-lg text-xs font-bold bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[var(--bg-steel)] hover:text-[var(--text-on-accent)] transition-all disabled:opacity-50"
                  >
                    Reload From Server
                  </button>
                  <button
                    type="button"
                    onClick={resetPromptTemplateRemote}
                    disabled={promptLoading}
                    className="px-3 py-2 rounded-lg text-xs font-bold bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/1)] transition-all disabled:opacity-50"
                  >
                    Reset To Default
                  </button>
                </div>
              </div>

              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Change Summary (optional)
                </label>
                <input
                  type="text"
                  value={promptChangeSummary}
                  onChange={(e) => setPromptChangeSummary(e.target.value)}
                  placeholder="e.g., tighten tool authorization language"
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>

              <div className="flex items-center gap-3">
                <button
                  type="button"
                  onClick={updatePromptTemplateRemote}
                  disabled={promptLoading || promptTemplate.trim() === loadedPromptTemplate.trim()}
                  className={`px-4 py-2 rounded-lg text-sm font-bold transition-all ${
                    promptLoading || promptTemplate.trim() === loadedPromptTemplate.trim()
                      ? 'bg-[rgb(var(--surface-rgb)/0.35)] text-[var(--text-muted)] cursor-not-allowed'
                      : 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)]'
                  }`}
                >
                  Apply Persona
                </button>
                {promptTemplate.trim() !== loadedPromptTemplate.trim() && (
                  <span className="text-xs text-[rgb(var(--warning-rgb)/0.9)] font-bold uppercase tracking-wider">
                    Unsaved Persona Changes
                  </span>
                )}
              </div>

              {promptStatus && (
                <p className={`text-xs ${promptStatus.startsWith('Error:') ? 'text-[var(--danger)]' : 'text-[rgb(var(--success-rgb)/0.9)]'}`}>
                  {promptStatus}
                </p>
              )}
            </div>
          </section>

          {/* PERSONALIZATION PREFERENCES */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">face</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">Personalization (Style &amp; Tone)</h2>
            </div>

            <div className="space-y-4">
              <div className="text-xs text-[var(--text-secondary)]">
                These settings adjust how the Orchestrator responds (tone/style). They do <span className="font-bold">not</span> grant new capabilities.
                They are stored per <span className="font-mono">twin_id</span> (this desktop’s user id).
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                    Preferences Twin ID
                  </label>
                  <HoverTooltip
                    title="Preferences Twin ID"
                    description="Preferences are stored per twin_id (operator/user id). Default comes from localStorage 'pagi_user_id'."
                  >
                    <input
                      type="text"
                      value={preferencesTwinId}
                      onChange={(e) => {
                        setPreferencesTwinId(e.target.value);
                        setPrefsStatus('');
                      }}
                      className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
                      placeholder="twin-aegis"
                    />
                  </HoverTooltip>
                  <p className="text-[10px] text-[var(--text-secondary)] mt-2">
                    Current app user id: <span className="font-mono font-bold">{localStorage.getItem('pagi_user_id') || '(none yet)'}</span>
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                    Persona Preset
                  </label>
                  <select
                    value={prefs.persona_preset}
                    onChange={(e) => {
                      setPrefs(prev => ({ ...prev, persona_preset: e.target.value }));
                      setPrefsDirty(true);
                    }}
                    className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                    disabled={prefsLoading}
                  >
                    {(personaPresets.length ? personaPresets : [{ id: 'default', label: 'Default', description: '', overlay: '' }]).map(p => (
                      <option key={p.id} value={p.id}>{p.label}</option>
                    ))}
                  </select>
                  {personaPresets.length > 0 && (
                    <p className="text-[10px] text-[var(--text-secondary)] mt-2">
                      {personaPresets.find(p => p.id === prefs.persona_preset)?.description || ''}
                    </p>
                  )}
                </div>

                <div>
                  <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                    Verbosity
                  </label>
                  <select
                    value={prefs.verbosity}
                    onChange={(e) => {
                      setPrefs(prev => ({ ...prev, verbosity: e.target.value as Verbosity }));
                      setPrefsDirty(true);
                    }}
                    className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                    disabled={prefsLoading}
                  >
                    <option value="minimal">Minimal</option>
                    <option value="balanced">Balanced</option>
                    <option value="detailed">Detailed</option>
                  </select>
                </div>

                <div className="flex items-center gap-4 pt-6">
                  <label className="flex items-center gap-2 text-xs text-[var(--text-secondary)] font-bold uppercase tracking-wider">
                    <input
                      type="checkbox"
                      checked={prefs.enable_cynical}
                      onChange={(e) => {
                        setPrefs(prev => ({ ...prev, enable_cynical: e.target.checked }));
                        setPrefsDirty(true);
                      }}
                      className="w-5 h-5 rounded border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)]"
                      disabled={prefsLoading}
                    />
                    Cynical
                  </label>
                  <label className="flex items-center gap-2 text-xs text-[var(--text-secondary)] font-bold uppercase tracking-wider">
                    <input
                      type="checkbox"
                      checked={prefs.enable_sarcastic}
                      onChange={(e) => {
                        setPrefs(prev => ({ ...prev, enable_sarcastic: e.target.checked }));
                        setPrefsDirty(true);
                      }}
                      className="w-5 h-5 rounded border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)]"
                      disabled={prefsLoading}
                    />
                    Sarcastic
                  </label>
                </div>
              </div>

              <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.2)] pt-4">
                <h3 className="text-sm font-bold text-[var(--text-secondary)] mb-3 uppercase tracking-wider">Operator Profile</h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-xs font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">Nickname</label>
                    <input
                      type="text"
                      value={prefs.profile.nickname}
                      onChange={(e) => {
                        setPrefs(prev => ({ ...prev, profile: { ...prev.profile, nickname: e.target.value } }));
                        setPrefsDirty(true);
                      }}
                      className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                      placeholder="Guy Fawkes"
                      disabled={prefsLoading}
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">Occupation</label>
                    <input
                      type="text"
                      value={prefs.profile.occupation}
                      onChange={(e) => {
                        setPrefs(prev => ({ ...prev, profile: { ...prev.profile, occupation: e.target.value } }));
                        setPrefsDirty(true);
                      }}
                      className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                      placeholder="Cybersecurity Manager"
                      disabled={prefsLoading}
                    />
                  </div>
                  <div className="md:col-span-2">
                    <label className="block text-xs font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">More about you</label>
                    <textarea
                      value={prefs.profile.about}
                      onChange={(e) => {
                        setPrefs(prev => ({ ...prev, profile: { ...prev.profile, about: e.target.value } }));
                        setPrefsDirty(true);
                      }}
                      rows={4}
                      className="w-full px-4 py-3 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                      placeholder="Agentic AI Project, Vulnerability Management"
                      disabled={prefsLoading}
                    />
                  </div>
                </div>
              </div>

              <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.2)] pt-4">
                <h3 className="text-sm font-bold text-[var(--text-secondary)] mb-3 uppercase tracking-wider">Custom Instructions</h3>
                <textarea
                  value={prefs.custom_instructions}
                  onChange={(e) => {
                    setPrefs(prev => ({ ...prev, custom_instructions: e.target.value }));
                    setPrefsDirty(true);
                  }}
                  rows={6}
                  className="w-full px-4 py-3 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
                  placeholder="Additional behavior, style, and tone preferences..."
                  disabled={prefsLoading}
                />
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onClick={() => fetchPreferencesForTwin(preferencesTwinId)}
                    disabled={prefsLoading}
                    className="px-3 py-2 rounded-lg text-xs font-bold bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[var(--bg-steel)] hover:text-[var(--text-on-accent)] transition-all disabled:opacity-50"
                  >
                    Reload From Server
                  </button>
                  <button
                    type="button"
                    onClick={savePreferences}
                    disabled={prefsLoading || !prefsDirty}
                    className={`px-4 py-2 rounded-lg text-sm font-bold transition-all ${
                      prefsLoading || !prefsDirty
                        ? 'bg-[rgb(var(--surface-rgb)/0.35)] text-[var(--text-muted)] cursor-not-allowed'
                        : 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)]'
                    }`}
                  >
                    Save Preferences
                  </button>
                  {prefsDirty && (
                    <span className="text-xs text-[rgb(var(--warning-rgb)/0.9)] font-bold uppercase tracking-wider">
                      Unsaved Changes
                    </span>
                  )}
                </div>
                {prefs.updated_at && (
                  <p className="text-[10px] text-[var(--text-secondary)] mt-2">
                    Last updated: <span className="font-mono">{prefs.updated_at}</span>
                  </p>
                )}
                {prefsStatus && (
                  <p className={`text-xs mt-2 ${prefsStatus.startsWith('Error:') ? 'text-[var(--danger)]' : 'text-[rgb(var(--success-rgb)/0.9)]'}`}>
                    {prefsStatus}
                  </p>
                )}
              </div>
            </div>
          </section>

          {/* API SETTINGS */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">api</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">API Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  OpenRouter API Key
                </label>
                <input
                  type="password"
                  value={settings.openrouterApiKey}
                  onChange={(e) => handleChange('openrouterApiKey', e.target.value)}
                  placeholder="sk-or-..."
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  OpenRouter Model
                </label>
                <input
                  type="text"
                  value={settings.openrouterModel}
                  onChange={(e) => handleChange('openrouterModel', e.target.value)}
                  placeholder="anthropic/claude-3.5-sonnet"
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Memory Service gRPC URL
                </label>
                <input
                  type="text"
                  value={settings.memoryGrpcUrl}
                  onChange={(e) => handleChange('memoryGrpcUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Tools Service gRPC URL
                </label>
                <input
                  type="text"
                  value={settings.toolsGrpcUrl}
                  onChange={(e) => handleChange('toolsGrpcUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Qdrant URL
                </label>
                <input
                  type="text"
                  value={settings.qdrantUrl}
                  onChange={(e) => handleChange('qdrantUrl', e.target.value)}
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
            </div>
          </section>

          {/* EMAIL/TEAMS MONITORING */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">mail</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">Email & Teams Monitoring</h2>
            </div>
            <div className="space-y-4">
              <div className="text-xs text-[var(--text-secondary)] bg-[rgb(var(--bg-secondary-rgb)/0.3)] p-3 rounded-lg">
                <p className="font-bold mb-1">Microsoft Graph API OAuth Setup:</p>
                <ol className="list-decimal list-inside space-y-1 ml-2">
                  <li>Register an app in Azure Portal (portal.azure.com)</li>
                  <li>Add API permissions: Mail.Read, Mail.Send, Chat.Read, Chat.Send, User.Read</li>
                  <li>Create a client secret and copy the values below</li>
                  <li>Set redirect URI to: <code className="font-mono bg-[rgb(var(--surface-rgb)/0.6)] px-1 rounded">{emailTeamsConfig.redirect_uri}</code></li>
                  <li>After configuring, complete OAuth flow to get access tokens</li>
                </ol>
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
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
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
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
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
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
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
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
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
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
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                />
              </div>
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
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
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
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
                  className="px-4 py-2 rounded-lg text-sm font-bold bg-[var(--bg-steel)] text-[var(--text-on-accent)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] transition-all"
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
                  className="px-4 py-2 rounded-lg text-sm font-bold bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[var(--bg-steel)] hover:text-[var(--text-on-accent)] transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Start OAuth Flow
                </button>
              </div>
              {oauthStatus && (
                <p className={`text-xs ${oauthStatus.startsWith('Error') ? 'text-[var(--danger)]' : 'text-[var(--success)]'}`}>
                  {oauthStatus}
                </p>
              )}
              <div className="mt-4 p-3 bg-[rgb(var(--bg-secondary-rgb)/0.3)] rounded-lg">
                <p className="text-xs text-[var(--text-secondary)] mb-2">
                  <strong>Note:</strong> The OAuth flow will automatically exchange the authorization code for tokens.
                  You don't need to manually paste tokens anymore. If the automatic flow fails, you can still manually set tokens below.
                </p>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
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
                  className="w-full px-4 py-2 rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono text-xs"
                  rows={3}
                />
              </div>
            </div>
          </section>

          {/* LOGO SETTINGS */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">image</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">Logo Settings</h2>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-bold text-[var(--text-secondary)] mb-2 uppercase tracking-wider">
                  Custom Logo
                </label>
                <div className="flex items-center gap-4">
                  <div className="w-24 h-24 rounded-lg border-2 border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] flex items-center justify-center overflow-hidden">
                    {logoUrl ? (
                      <img
                        src={logoUrl}
                        alt="Custom Logo"
                        className="w-full h-full object-contain"
                        onError={() => setLogoUrl('/ferrellgas-agi-badge.svg')}
                      />
                    ) : (
                      <span className="material-symbols-outlined text-[var(--bg-steel)]">image</span>
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
                          ? 'bg-[rgb(var(--surface-rgb)/0.35)] text-[var(--text-muted)] cursor-not-allowed'
                          : 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)]'
                      }`}
                    >
                      {uploading ? 'Uploading...' : 'Upload Logo'}
                    </label>
                    {uploadStatus && (
                      <p className={`text-xs mt-2 ${uploadStatus.startsWith('Error') ? 'text-[var(--danger)]' : 'text-[var(--success)]'}`}>
                        {uploadStatus}
                      </p>
                    )}
                    <p className="text-xs text-[var(--text-secondary)] mt-2">
                      Supported formats: PNG, JPG, SVG. Max size: 5MB
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </section>

          {/* SECURITY GATES / ENVIRONMENT VARIABLES */}
          <section className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">security</span>
              <h2 className="text-lg font-bold text-[var(--text-primary)]">Security Gates (Research Project Access)</h2>
            </div>

            <div className="space-y-4">
              <div className="text-xs text-[var(--text-secondary)] bg-[rgb(var(--warning-rgb)/0.15)] border border-[rgb(var(--warning-rgb)/0.3)] rounded-lg p-3">
                <p className="font-bold mb-1 text-[rgb(var(--warning-rgb)/0.98)]">⚠️ WARNING: Research Project Only</p>
                <p className="text-[rgb(var(--warning-rgb)/0.95)]">
                  These security gates bypass normal safety restrictions. Use only in isolated research environments.
                  Changes are written directly to the <code className="font-mono bg-[rgb(var(--surface-rgb)/0.6)] px-1 rounded">.env</code> file.
                  Services may need to be restarted for changes to take effect.
                </p>
              </div>

              {envFilePath && (
                <div className="text-xs text-[var(--text-secondary)] bg-[rgb(var(--bg-secondary-rgb)/0.3)] p-2 rounded-lg">
                  <span className="font-bold">.env file location:</span>{' '}
                  <code className="font-mono bg-[rgb(var(--surface-rgb)/0.6)] px-1 rounded">{envFilePath}</code>
                </div>
              )}

              <div className="flex items-center gap-2 mb-4">
                <button
                  type="button"
                  onClick={fetchEnvFile}
                  disabled={envLoading}
                  className="px-3 py-2 rounded-lg text-xs font-bold bg-[var(--bg-muted)] text-[var(--text-primary)] hover:bg-[var(--bg-steel)] hover:text-[var(--text-on-accent)] transition-all disabled:opacity-50"
                >
                  Reload From Server
                </button>
                <button
                  type="button"
                  onClick={handleSaveEnvFile}
                  disabled={envLoading || !envHasChanges}
                  className={`px-4 py-2 rounded-lg text-sm font-bold transition-all ${
                    envLoading || !envHasChanges
                      ? 'bg-[rgb(var(--surface-rgb)/0.35)] text-[var(--text-muted)] cursor-not-allowed'
                      : 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)]'
                  }`}
                >
                  Save Environment Variables
                </button>
                {envHasChanges && (
                  <span className="text-xs text-[rgb(var(--warning-rgb)/0.9)] font-bold uppercase tracking-wider">
                    Unsaved Changes
                  </span>
                )}
              </div>

              {envStatus && (
                <p className={`text-xs ${envStatus.startsWith('Error:') ? 'text-[var(--danger)]' : 'text-[rgb(var(--success-rgb)/0.9)]'}`}>
                  {envStatus}
                </p>
              )}

              {/* Security Gates by Category */}
              {['network', 'hitl', 'communication', 'commands'].map((category) => {
                const categoryGates = SECURITY_GATES.filter((gate) => gate.category === category);
                if (categoryGates.length === 0) return null;

                const categoryLabels: Record<string, string> = {
                  network: 'Network Scanning',
                  hitl: 'HITL (Human-In-The-Loop) Bypass',
                  communication: 'Communication',
                  commands: 'Restricted Commands',
                };

                return (
                  <div key={category} className="border-t border-[rgb(var(--bg-steel-rgb)/0.2)] pt-4">
                    <h3 className="text-sm font-bold text-[var(--text-secondary)] mb-3 uppercase tracking-wider">
                      {categoryLabels[category]}
                    </h3>
                    <div className="space-y-3">
                      {categoryGates.map((gate) => {
                        const isEnabled = isSecurityGateEnabled(envVars, gate.key);
                        const isCritical = gate.description.includes('CRITICAL');

                        return (
                          <div
                            key={gate.key}
                            className={`p-3 rounded-lg border ${
                              isCritical
                                ? 'bg-[rgb(var(--danger-rgb)/0.08)] border-[rgb(var(--danger-rgb)/0.3)]'
                                : 'bg-[rgb(var(--surface-rgb)/0.4)] border-[rgb(var(--bg-steel-rgb)/0.2)]'
                            }`}
                          >
                            <div className="flex items-start justify-between gap-3">
                              <div className="flex-1">
                                <div className="flex items-center gap-2 mb-1">
                                  <label
                                    htmlFor={`gate-${gate.key}`}
                                    className="text-sm font-bold text-[var(--text-primary)] cursor-pointer"
                                  >
                                    {gate.label}
                                  </label>
                                  {isCritical && (
                                    <span className="text-[10px] font-black text-[rgb(var(--danger-rgb)/0.85)] uppercase tracking-wider bg-[rgb(var(--danger-rgb)/0.15)] px-1.5 py-0.5 rounded">
                                      CRITICAL
                                    </span>
                                  )}
                                </div>
                                <p className="text-xs text-[var(--text-secondary)] leading-relaxed">{gate.description}</p>
                              </div>
                              <div className="flex items-center">
                                <input
                                  type="checkbox"
                                  id={`gate-${gate.key}`}
                                  checked={isEnabled}
                                  onChange={(e) => handleSecurityGateToggle(gate.key, e.target.checked)}
                                  className="w-5 h-5 rounded border-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--bg-steel)] focus:ring-2 focus:ring-[var(--bg-steel)] cursor-pointer"
                                />
                              </div>
                            </div>
                            {gate.key === 'ALLOW_PUBLIC_NETWORK_SCAN' && isEnabled && (
                              <div className="mt-2 pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                                <label className="block text-xs font-bold text-[var(--text-secondary)] mb-1">
                                  NETWORK_SCAN_HITL_TOKEN
                                </label>
                                <input
                                  type="text"
                                  value={envVars['NETWORK_SCAN_HITL_TOKEN'] || ''}
                                  onChange={(e) => {
                                    const newEnvVars = { ...envVars };
                                    newEnvVars['NETWORK_SCAN_HITL_TOKEN'] = e.target.value;
                                    setEnvVars(newEnvVars);
                                    setEnvHasChanges(true);
                                  }}
                                  placeholder="Enter HITL token for public scans"
                                  className="w-full px-3 py-1.5 rounded border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] text-xs focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono"
                                />
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
};

export default OrchestratorSettings;
