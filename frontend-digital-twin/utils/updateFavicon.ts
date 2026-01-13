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

  // Browsers cache favicons aggressively; changing the href to the same URL
  // often does nothing. Always cache-bust when we switch to a custom asset.
  const bust = `v=${Date.now()}`;

  const replaceLink = (link: Element, attrs: Record<string, string>) => {
    const el = link as HTMLLinkElement;
    const cloned = el.cloneNode(true) as HTMLLinkElement;
    Object.entries(attrs).forEach(([k, v]) => cloned.setAttribute(k, v));
    el.parentNode?.replaceChild(cloned, el);
  };

  if (faviconIco) {
    const img = new Image();
    img.onload = () => {
      replaceLink(faviconIco, {
        href: `${getCustomFaviconUrl()}?${bust}`,
      });
    };
    img.onerror = () => {
      // Keep default if custom doesn't exist
    };
    img.src = `${getCustomFaviconUrl()}?${bust}`;
  }

  if (faviconPng) {
    const img = new Image();
    img.onload = () => {
      // If we have a custom PNG favicon, prefer it over the default SVG favicon.
      // Many browsers will prefer SVG when present.
      if (faviconSvg) {
        faviconSvg.parentNode?.removeChild(faviconSvg);
      }

      replaceLink(faviconPng, {
        href: `${getCustomFaviconPngUrl()}?${bust}`,
        rel: 'icon',
        type: 'image/png',
        sizes: '32x32',
      });
    };
    img.onerror = () => {
      // Keep default if custom doesn't exist
    };
    img.src = `${getCustomFaviconPngUrl()}?${bust}`;
  }

  // Leave the SVG favicon alone unless a custom PNG exists (handled above).
}
