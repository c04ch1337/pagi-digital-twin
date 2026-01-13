import React, { useState, useEffect } from 'react';
import { uploadAsset, getCustomLogoUrl, getAssetUrl, checkAssetExists } from '../services/assetService';
import HoverTooltip from './HoverTooltip';
import { updateFaviconLinks } from '../utils/updateFavicon';

interface RootAdminSettingsProps {
  onClose: () => void;
}

interface RootAdminSettingsData {
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

const RootAdminSettings: React.FC<RootAdminSettingsProps> = ({ onClose }) => {
  // Load settings from localStorage
  const loadSettings = (): RootAdminSettingsData => {
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

  const [settings, setSettings] = useState<RootAdminSettingsData>(loadSettings);
  const [logoUrl, setLogoUrl] = useState(settings.customLogoUrl);
  const [uploading, setUploading] = useState(false);
  const [uploadStatus, setUploadStatus] = useState<string>('');
  const [hasChanges, setHasChanges] = useState(false);

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

  const handleChange = (key: keyof RootAdminSettingsData, value: string) => {
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
          console.warn('[RootAdminSettings] Favicon generation/upload skipped:', e);
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
    return settings.userName.trim() || 'ROOT ADMIN';
  };

  return (
    <div className="h-full flex flex-col bg-[#9EC9D9]">
      {/* Header */}
      <div className="border-b border-[#5381A5]/30 bg-[#90C3EA] px-6 py-4 flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-[#0b1b2b]">Root Admin Settings</h1>
          <p className="text-xs text-[#163247] mt-1">Configure system-wide settings and preferences</p>
        </div>
        <div className="flex items-center gap-3">
          {hasChanges && (
            <span className="text-xs text-amber-600 font-bold uppercase tracking-wider">Unsaved Changes</span>
          )}
          <button
            onClick={handleSave}
            disabled={!hasChanges}
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
                  description="Enter your name so The Blue Flame orchestrator can address you personally. If left empty, it will use 'ROOT ADMIN' as the default."
                >
                  <input
                    type="text"
                    value={settings.userName}
                    onChange={(e) => handleChange('userName', e.target.value)}
                    placeholder="ROOT ADMIN"
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

export default RootAdminSettings;
