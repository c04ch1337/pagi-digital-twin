/*
  Refactor hardcoded, non-theme-aware color utilities into theme tokens.

  Usage:
    node scripts/refactor-theme-colors.mjs --write
    node scripts/refactor-theme-colors.mjs            (dry-run)

  This is intentionally conservative: it only replaces patterns that are
  unambiguous and already have canonical tokens in `index.css`.
*/

import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(process.cwd());
const WRITE = process.argv.includes('--write');

const INCLUDE_EXT = new Set(['.ts', '.tsx', '.js', '.jsx', '.css']);
const SKIP_DIRS = new Set(['node_modules', '.next', 'dist', 'build']);

/** @type {{name: string, from: RegExp, to: string}[]} */
const rules = [
  {
    name: 'accent hover background (#437091, #3d6a8a) -> var(--accent-hover)',
    from: /\bhover:bg-\[#(?:437091|3d6a8a)\]\b/g,
    to: 'hover:bg-[var(--accent-hover)]',
  },
  {
    name: 'text-white -> text-on-accent',
    from: /\btext-white\b/g,
    to: 'text-[var(--text-on-accent)]',
  },
  {
    name: 'hover:text-white -> hover:text-on-accent',
    from: /\bhover:text-white\b/g,
    to: 'hover:text-[var(--text-on-accent)]',
  },
  {
    name: 'file:text-white -> file:text-on-accent',
    from: /\bfile:text-white\b/g,
    to: 'file:text-[var(--text-on-accent)]',
  },
  // --- Status colors (Tailwind palette → semantic tokens) ---
  {
    name: 'text-rose-600 -> danger',
    from: /\btext-rose-600\b/g,
    to: 'text-[var(--danger)]',
  },
  {
    name: 'hover:text-rose-700 -> danger (strong)',
    from: /\bhover:text-rose-700\b/g,
    to: 'hover:text-[rgb(var(--danger-rgb)/0.9)]',
  },
  {
    name: 'text-emerald-600 -> success',
    from: /\btext-emerald-600\b/g,
    to: 'text-[var(--success)]',
  },
  {
    name: 'text-emerald-700 -> success (strong)',
    from: /\btext-emerald-700\b/g,
    to: 'text-[rgb(var(--success-rgb)/0.9)]',
  },
  {
    name: 'text-amber-600 -> warning',
    from: /\btext-amber-600\b/g,
    to: 'text-[var(--warning)]',
  },
  {
    name: 'text-amber-700 -> warning (strong)',
    from: /\btext-amber-700\b/g,
    to: 'text-[rgb(var(--warning-rgb)/0.9)]',
  },
  {
    name: 'text-amber-800 -> warning (stronger)',
    from: /\btext-amber-800\b/g,
    to: 'text-[rgb(var(--warning-rgb)/0.95)]',
  },
  {
    name: 'text-amber-900 -> warning (max)',
    from: /\btext-amber-900\b/g,
    to: 'text-[rgb(var(--warning-rgb)/0.98)]',
  },
  {
    name: 'bg-amber-100/60 -> warning tint',
    from: /\bbg-amber-100\/60\b/g,
    to: 'bg-[rgb(var(--warning-rgb)/0.15)]',
  },
  {
    name: 'border-amber-300/60 -> warning border',
    from: /\bborder-amber-300\/60\b/g,
    to: 'border-[rgb(var(--warning-rgb)/0.3)]',
  },
  {
    name: 'bg-rose-50/50 -> danger tint',
    from: /\bbg-rose-50\/50\b/g,
    to: 'bg-[rgb(var(--danger-rgb)/0.08)]',
  },
  {
    name: 'border-rose-300/60 -> danger border',
    from: /\bborder-rose-300\/60\b/g,
    to: 'border-[rgb(var(--danger-rgb)/0.3)]',
  },
  {
    name: 'hover:border-rose-500/40 -> danger border hover',
    from: /\bhover:border-rose-500\/40\b/g,
    to: 'hover:border-[rgb(var(--danger-rgb)/0.4)]',
  },
  {
    name: 'bg-gray-300 -> disabled surface',
    from: /\bbg-gray-300\b/g,
    to: 'bg-[rgb(var(--surface-rgb)/0.35)]',
  },
  {
    name: 'shadow-black/20 -> overlay shadow color',
    from: /\bshadow-black\/20\b/g,
    to: '[--tw-shadow-color:rgb(var(--overlay-rgb)/0.2)]',
  },
  {
    name: 'shadow-black/30 -> overlay shadow color (strong)',
    from: /\bshadow-black\/30\b/g,
    to: '[--tw-shadow-color:rgb(var(--overlay-rgb)/0.3)]',
  },
  {
    name: 'border-indigo-500 -> accent border',
    from: /\bborder-indigo-500\b/g,
    to: 'border-[var(--accent)]',
  },

  // --- More status colors seen in dashboards/telemetry ---
  {
    name: 'text-cyan-400 -> info',
    from: /\btext-cyan-400\b/g,
    to: 'text-[var(--info)]',
  },
  {
    name: 'text-cyan-300 -> info (muted)',
    from: /\btext-cyan-300\b/g,
    to: 'text-[rgb(var(--info-rgb)/0.9)]',
  },
  {
    name: 'text-cyan-300/80 -> info (muted)',
    from: /\btext-cyan-300\/80\b/g,
    to: 'text-[rgb(var(--info-rgb)/0.8)]',
  },
  {
    name: 'border-cyan-500/30 -> info border',
    from: /\bborder-cyan-500\/30\b/g,
    to: 'border-[rgb(var(--info-rgb)/0.3)]',
  },
  {
    name: 'bg-cyan-500 -> info bg',
    from: /\bbg-cyan-500\b/g,
    to: 'bg-[var(--info)]',
  },

  {
    name: 'text-yellow-600 -> warning',
    from: /\btext-yellow-600\b/g,
    to: 'text-[var(--warning)]',
  },
  {
    name: 'text-yellow-500 -> warning (muted)',
    from: /\btext-yellow-500\b/g,
    to: 'text-[rgb(var(--warning-rgb)/0.9)]',
  },
  {
    name: 'text-yellow-400 -> warning (muted)',
    from: /\btext-yellow-400\b/g,
    to: 'text-[rgb(var(--warning-rgb)/0.85)]',
  },
  {
    name: 'text-yellow-200 -> warning (very muted)',
    from: /\btext-yellow-200\b/g,
    to: 'text-[rgb(var(--warning-rgb)/0.7)]',
  },
  {
    name: 'bg-yellow-900/20 -> warning bg',
    from: /\bbg-yellow-900\/20\b/g,
    to: 'bg-[rgb(var(--warning-rgb)/0.2)]',
  },
  {
    name: 'bg-yellow-500/20 -> warning bg',
    from: /\bbg-yellow-500\/20\b/g,
    to: 'bg-[rgb(var(--warning-rgb)/0.2)]',
  },
  {
    name: 'border-yellow-600 -> warning border',
    from: /\bborder-yellow-600\b/g,
    to: 'border-[rgb(var(--warning-rgb)/0.6)]',
  },
  {
    name: 'border-yellow-500/50 -> warning border',
    from: /\bborder-yellow-500\/50\b/g,
    to: 'border-[rgb(var(--warning-rgb)/0.5)]',
  },

  {
    name: 'text-red-600 -> danger',
    from: /\btext-red-600\b/g,
    to: 'text-[var(--danger)]',
  },
  {
    name: 'text-red-500 -> danger (muted)',
    from: /\btext-red-500\b/g,
    to: 'text-[rgb(var(--danger-rgb)/0.9)]',
  },
  {
    name: 'text-red-400 -> danger (muted)',
    from: /\btext-red-400\b/g,
    to: 'text-[rgb(var(--danger-rgb)/0.8)]',
  },
  {
    name: 'text-red-200 -> danger (very muted)',
    from: /\btext-red-200\b/g,
    to: 'text-[rgb(var(--danger-rgb)/0.65)]',
  },
  {
    name: 'bg-red-900/20 -> danger bg',
    from: /\bbg-red-900\/20\b/g,
    to: 'bg-[rgb(var(--danger-rgb)/0.2)]',
  },
  {
    name: 'border-red-600 -> danger border',
    from: /\bborder-red-600\b/g,
    to: 'border-[rgb(var(--danger-rgb)/0.6)]',
  },
  {
    name: 'border-red-300/50 -> danger border',
    from: /\bborder-red-300\/50\b/g,
    to: 'border-[rgb(var(--danger-rgb)/0.3)]',
  },
  {
    name: 'border-red-300/60 -> danger border',
    from: /\bborder-red-300\/60\b/g,
    to: 'border-[rgb(var(--danger-rgb)/0.35)]',
  },

  {
    name: 'text-green-400 -> success',
    from: /\btext-green-400\b/g,
    to: 'text-[rgb(var(--success-rgb)/0.85)]',
  },
  {
    name: 'text-green-500 -> success',
    from: /\btext-green-500\b/g,
    to: 'text-[var(--success)]',
  },
  {
    name: 'border-green-600 -> success border',
    from: /\bborder-green-600\b/g,
    to: 'border-[rgb(var(--success-rgb)/0.6)]',
  },
];

function walk(dir) {
  /** @type {string[]} */
  const out = [];
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP_DIRS.has(ent.name)) continue;
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) out.push(...walk(p));
    else if (INCLUDE_EXT.has(path.extname(ent.name))) out.push(p);
  }
  return out;
}

let changedFiles = 0;
let totalReplacements = 0;

for (const file of walk(ROOT)) {
  const before = fs.readFileSync(file, 'utf8');
  let after = before;
  let fileReplacements = 0;

  for (const r of rules) {
    const prev = after;
    after = after.replace(r.from, () => {
      fileReplacements += 1;
      totalReplacements += 1;
      return r.to;
    });
    // If rule didn't apply, keep going.
    if (prev === after) continue;
  }

  if (after !== before) {
    changedFiles += 1;
    if (WRITE) {
      try {
        // On Windows, some editors/watchers can transiently lock files.
        // Use a small retry loop instead of failing the whole run.
        const maxAttempts = 5;
        let lastErr;
        for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
          try {
            fs.writeFileSync(file, after, 'utf8');
            lastErr = null;
            break;
          } catch (e) {
            lastErr = e;
            // backoff: 25ms, 50ms, 75ms, ...
            Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, attempt * 25);
          }
        }
        if (lastErr) throw lastErr;
      } catch (e) {
        console.warn(`FAILED\t${path.relative(ROOT, file)}\t${e?.code || ''} ${e?.message || e}`);
        // Do not crash the whole refactor run.
        continue;
      }
    }
    const rel = path.relative(ROOT, file);
    console.log(`${WRITE ? 'UPDATED' : 'WOULD UPDATE'}\t${rel}\t(${fileReplacements} replacements)`);
  }
}

console.log(`\n${WRITE ? 'Applied' : 'Planned'} ${totalReplacements} replacements across ${changedFiles} files.`);

