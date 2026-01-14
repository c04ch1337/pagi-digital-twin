/**
 * Utility functions for formatting metrics with compact notation
 * to improve scannability and reduce visual noise
 */

/**
 * Formats a number with compact notation (e.g., 1242 -> "1.2k", 1500000 -> "1.5M")
 */
export function formatCompactNumber(num: number): string {
  if (!Number.isFinite(num)) return '0';
  
  const abs = Math.abs(num);
  
  if (abs < 1000) {
    return num.toString();
  }
  
  if (abs < 1000000) {
    const k = num / 1000;
    return `${k.toFixed(1)}k`;
  }
  
  if (abs < 1000000000) {
    const m = num / 1000000;
    return `${m.toFixed(1)}M`;
  }
  
  const b = num / 1000000000;
  return `${b.toFixed(1)}B`;
}

/**
 * Formats bytes with compact notation (e.g., 1242 -> "1.2 KB", 1500000 -> "1.5 MB")
 */
export function formatCompactBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B';
  
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  
  if (bytes < 1024 * 1024) {
    const kb = bytes / 1024;
    return `${kb.toFixed(1)} KB`;
  }
  
  if (bytes < 1024 * 1024 * 1024) {
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(1)} MB`;
  }
  
  const gb = bytes / (1024 * 1024 * 1024);
  return `${gb.toFixed(2)} GB`;
}

/**
 * Formats KiB with compact notation (e.g., 1024 -> "1.0 MiB")
 */
export function formatCompactKiB(kib: number): string {
  if (!Number.isFinite(kib) || kib < 0) return '0 KiB';
  
  const mib = kib / 1024;
  if (mib < 1024) {
    return `${mib.toFixed(1)} MiB`;
  }
  
  const gib = mib / 1024;
  return `${gib.toFixed(2)} GiB`;
}
