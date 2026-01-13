/**
 * Utility to update favicon links dynamically
 */

import { getCustomFaviconUrl, getCustomFaviconPngUrl } from '../services/assetService';

/**
 * Update favicon links in the document head to use custom favicons if available
 */
export function updateFaviconLinks() {
  // Try to load custom favicons, fallback to defaults
  const faviconIco = document.querySelector('link[rel="icon"][sizes="any"]');
  const faviconPng = document.querySelector('link[rel="icon"][type="image/png"]');
  const faviconSvg = document.querySelector('link[rel="icon"][type="image/svg+xml"]');

  if (faviconIco) {
    const img = new Image();
    img.onload = () => {
      faviconIco.setAttribute('href', getCustomFaviconUrl());
    };
    img.onerror = () => {
      // Keep default if custom doesn't exist
    };
    img.src = getCustomFaviconUrl();
  }

  if (faviconPng) {
    const img = new Image();
    img.onload = () => {
      faviconPng.setAttribute('href', getCustomFaviconPngUrl());
    };
    img.onerror = () => {
      // Keep default if custom doesn't exist
    };
    img.src = getCustomFaviconPngUrl();
  }
}
