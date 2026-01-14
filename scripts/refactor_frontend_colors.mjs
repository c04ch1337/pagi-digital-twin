import fs from 'node:fs';
import path from 'node:path';

// Refactors hardcoded Tailwind colors in `frontend-digital-twin/**` to CSS-variable based tokens.
//
// Design goals:
// - Only touch files in `frontend-digital-twin`.
// - Prefer safe, syntactic replacements (Tailwind class tokens) over global string/hex rewriting.
// - Support Tailwind arbitrary colors with optional opacity: `bg-[#5381A5]/30`.
// - Map Tailwind palette classes used in a few components (gray/blue/red) to theme variables.

const FRONTEND_ROOT = path.resolve('frontend-digital-twin');

/** @param {string} s */
function escapeRegExp(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** @param {string} p */
function walk(p) {
  /** @type {string[]} */
  const out = [];
  const entries = fs.readdirSync(p, { withFileTypes: true });
  for (const e of entries) {
    if (e.name === 'node_modules' || e.name === 'dist' || e.name === '.git') continue;
    const full = path.join(p, e.name);
    if (e.isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

function pctToAlpha(pct) {
  const n = Number(pct);
  if (!Number.isFinite(n) || n < 0) return null;
  if (n > 100) return null;
  // Tailwind uses percent-style integers: 30 => 0.3
  return (n / 100).toString().replace(/0+$/, '').replace(/\.$/, '');
}

/**
 * Known theme hex -> token name mapping.
 * Keep keys lowercase.
 */
const HEX_TO_TOKEN = {
  '9ec9d9': 'bg-primary',
  '90c3ea': 'bg-secondary',
  '78a2c2': 'bg-muted',
  '5381a5': 'bg-steel',
  '0b1b2b': 'text-primary',
  '163247': 'text-secondary',
  '214a6b': 'bg-muted',
};

/**
 * Tailwind arbitrary color target utilities we transform.
 * Example: `bg-[#5381A5]/30`.
 */
const ARB_COLOR_RE = /(\b(?:bg|text|border|ring|from|via|to|shadow|stroke|fill|placeholder|accent|caret)-)\[#([0-9a-fA-F]{6})\](?:\/([0-9]{1,3}))?/g;

/**
 * A small set of Tailwind palette classes we map to semantic variables.
 * This avoids hardcoded Tailwind palette colors.
 */
const SIMPLE_CLASS_REPLACEMENTS = [
  // Brand / primary accent
  ['bg-blue-600', 'bg-[var(--accent)]'],
  ['hover:bg-blue-600', 'hover:bg-[var(--accent)]'],
  ['text-blue-600', 'text-[var(--accent)]'],

  // Grays -> theme surfaces
  ['bg-gray-950', 'bg-[var(--bg-primary)]'],
  ['bg-gray-900', 'bg-[var(--bg-secondary)]'],
  ['bg-gray-800', 'bg-[var(--bg-muted)]'],
  ['bg-gray-700', 'bg-[rgb(var(--bg-muted-rgb)/0.9)]'],
  ['border-gray-800', 'border-[var(--border-color)]'],
  ['border-gray-700', 'border-[var(--border-color)]'],
  ['text-gray-500', 'text-[var(--text-muted)]'],
  ['text-gray-400', 'text-[var(--text-muted)]'],
  ['text-gray-300', 'text-[var(--text-secondary)]'],
  ['text-gray-200', 'text-[var(--text-primary)]'],

  // Whites/blacks -> theme-aware surfaces/overlays
  ['bg-white', 'bg-[rgb(var(--surface-rgb)/1)]'],
  ['hover:bg-white', 'hover:bg-[rgb(var(--surface-rgb)/1)]'],
  ['text-white', 'text-[var(--text-on-accent)]'],
  ['border-white', 'border-[rgb(var(--text-on-accent-rgb)/1)]'],
  ['bg-black', 'bg-[rgb(var(--overlay-rgb)/1)]'],
  ['hover:bg-black', 'hover:bg-[rgb(var(--overlay-rgb)/1)]'],
  ['text-black', 'text-[rgb(var(--overlay-rgb)/1)]'],

  // Reds -> semantic danger
  ['bg-red-500', 'bg-[rgb(var(--danger-rgb)/1)]'],
  ['hover:bg-red-600', 'hover:bg-[rgb(var(--danger-rgb)/0.9)]'],
  ['bg-red-50', 'bg-[rgb(var(--danger-rgb)/0.12)]'],
  ['text-red-900', 'text-[rgb(var(--danger-rgb)/0.95)]'],
  ['text-red-700', 'text-[rgb(var(--danger-rgb)/0.85)]'],
  ['border-red-300', 'border-[rgb(var(--danger-rgb)/0.35)]'],
  ['text-rose-700', 'text-[rgb(var(--danger-rgb)/0.85)]'],
  ['bg-rose-600', 'bg-[rgb(var(--danger-rgb)/0.95)]'],
  ['hover:bg-rose-700', 'hover:bg-[rgb(var(--danger-rgb)/0.85)]'],
  ['border-rose-300', 'border-[rgb(var(--danger-rgb)/0.35)]'],
];

/** Tailwind fraction classes like `bg-white/70`, `text-white/60`, `border-white/20`, `bg-black/50`. */
const FRACTION_RE = /(\b(?:bg|text|border)-(?:white|black))\/([0-9]{1,3})\b/g;

function applyReplacements(input) {
  let out = input;

  // 1) Replace Tailwind palette class occurrences.
  for (const [from, to] of SIMPLE_CLASS_REPLACEMENTS) {
    // Replace class tokens even when they're adjacent to quotes/backticks (common in className strings).
    const re = new RegExp(
      '(^|[\\s"\'`\\{\\(\\[] )'
        .replace(' ', '') +
        escapeRegExp(from) +
        '(?=([\\s"\'`\\}\\)\\]]|$))',
      'g'
    );
    out = out.replace(re, `$1${to}`);
  }

  // 2) Replace Tailwind arbitrary hex colors (optionally with /opacity).
  out = out.replace(ARB_COLOR_RE, (match, prefix, hex, pct) => {
    const token = HEX_TO_TOKEN[String(hex).toLowerCase()];
    if (!token) return match;

    const baseVar = `--${token}`;
    if (!pct) {
      return `${prefix}[var(${baseVar})]`;
    }

    const alpha = pctToAlpha(pct);
    if (!alpha) return match;

    const rgbVar = `--${token}-rgb`;
    return `${prefix}[rgb(var(${rgbVar})/${alpha})]`;
  });

  // 3) Replace `white/NN` and `black/NN` fractions with theme-aware vars.
  out = out.replace(FRACTION_RE, (match, util, pct) => {
    const alpha = pctToAlpha(pct);
    if (!alpha) return match;
    const isWhite = util.endsWith('white');
    const isBlack = util.endsWith('black');
    const prefix = util.startsWith('bg-') ? 'bg-' : util.startsWith('text-') ? 'text-' : 'border-';

    if (isWhite) {
      if (prefix === 'text-') return `text-[rgb(var(--text-on-accent-rgb)/${alpha})]`;
      if (prefix === 'border-') return `border-[rgb(var(--text-on-accent-rgb)/${alpha})]`;
      return `bg-[rgb(var(--surface-rgb)/${alpha})]`;
    }

    if (isBlack) {
      if (prefix === 'text-') return `text-[rgb(var(--overlay-rgb)/${alpha})]`;
      if (prefix === 'border-') return `border-[rgb(var(--overlay-rgb)/${alpha})]`;
      return `bg-[rgb(var(--overlay-rgb)/${alpha})]`;
    }

    return match;
  });

  return out;
}

const allowedExt = new Set(['.ts', '.tsx', '.css', '.html']);
const files = walk(FRONTEND_ROOT).filter((f) => allowedExt.has(path.extname(f)));

let changed = 0;
for (const file of files) {
  const before = fs.readFileSync(file, 'utf8');
  const after = applyReplacements(before);
  if (after !== before) {
    fs.writeFileSync(file, after, 'utf8');
    changed++;
  }
}

console.log(`[refactor_frontend_colors] Updated ${changed} files under ${FRONTEND_ROOT}`);

