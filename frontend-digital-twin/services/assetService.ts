/**
 * Asset Service
 * Handles uploading and retrieving custom branding assets (logo, favicon)
 */

const GATEWAY_URL = import.meta.env.VITE_GATEWAY_URL || 'http://127.0.0.1:8181';

export interface AssetUploadResponse {
  ok: boolean;
  asset_type: string;
  stored_path: string;
}

/**
 * Upload a custom asset (logo or favicon)
 */
export async function uploadAsset(
  file: File,
  assetType: 'logo' | 'favicon' | 'favicon-png'
): Promise<AssetUploadResponse> {
  const formData = new FormData();
  formData.append('file', file);
  formData.append('asset_type', assetType);

  const response = await fetch(`${GATEWAY_URL}/api/assets/upload`, {
    method: 'POST',
    body: formData,
  });

  if (!response.ok) {
    const errorText = await response.text().catch(() => 'Unknown error');
    throw new Error(`Failed to upload asset: ${response.statusText} - ${errorText}`);
  }

  return response.json();
}

/**
 * Get the URL for a custom asset
 */
export function getAssetUrl(filename: string): string {
  return `${GATEWAY_URL}/api/assets/${filename}`;
}

/**
 * Get the URL for custom logo
 */
export function getCustomLogoUrl(): string {
  return getAssetUrl('custom-logo.svg');
}

/**
 * Get the URL for custom favicon (ICO)
 */
export function getCustomFaviconUrl(): string {
  return getAssetUrl('custom-favicon.ico');
}

/**
 * Get the URL for custom favicon PNG (32x32)
 */
export function getCustomFaviconPngUrl(): string {
  return getAssetUrl('custom-favicon-32.png');
}

/**
 * Check if a custom asset exists by trying to load it
 */
export async function checkAssetExists(url: string): Promise<boolean> {
  try {
    const response = await fetch(url, { method: 'HEAD' });
    return response.ok;
  } catch {
    return false;
  }
}
