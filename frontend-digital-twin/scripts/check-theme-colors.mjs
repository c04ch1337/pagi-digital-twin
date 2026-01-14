/*
  Guardrail: fail CI/dev if hardcoded, non-theme-aware colors are reintroduced.

  Usage:
    node scripts/check-theme-colors.mjs

  What it checks for (outside allowed files):
    - Hex colors: #RGB[A] / #RRGGBB[AA]
    - rgb()/rgba()/hsl()/hsla() literals
    - Named colors that commonly sneak in: white/black

  Allowed locations:
    - `index.css` (canonical theme token definitions)
    - `public/` SVG assets (brand assets)
    - `scripts/refactor-theme-colors.mjs` (codemod contains literals)
*/

import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(process.cwd());

const INCLUDE_EXT = new Set(['.ts', '.tsx', '.js', '.jsx', '.css', '.html', '.svg']);
const SKIP_DIRS = new Set(['node_modules', '.next', 'dist', 'build', '.git']);

/** @param {string} p */
function normalize(p) {
  return p.split(path.sep).join('/');
}

/**
 * @param {string} rel
 * @returns {boolean}
 */
function isAllowed(rel) {
  const r = normalize(rel);
  if (r === 'index.css') return true;
  if (r.startsWith('public/') && r.endsWith('.svg')) return true;
  if (r === 'scripts/refactor-theme-colors.mjs') return true;
  return false;
}

/** @param {string} dir */
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

/**
 * Note: This intentionally ignores `rgb(var(--token-rgb) / <alpha>)` usage
 * because it is the preferred theme-aware pattern.
 */
const PATTERNS = [
  {
    name: 'hex',
    // Avoid matching inside urls like %23 in inline SVG data.
    re: /#[0-9a-fA-F]{3,8}\b/g,
  },
  {
    name: 'rgb/rgba numeric literal',
    // Flags rgb(255, 255, 255) / rgba(0,0,0,0.5). Does NOT flag template helpers like `rgba(${...})`.
    re: /\brgba?\(\s*\d/gi,
  },
  {
    name: 'hsl/hsla numeric literal',
    re: /\bhsla?\(\s*\d/gi,
  },
  {
    name: 'named colors',
    // Avoid false positives like Tailwind's `font-black`.
    re: /\b(?:text|bg|border)-(?:white|black)\b/gi,
  },
];

function lineColFromIndex(text, idx) {
  let line = 1;
  let lastNl = -1;
  for (let i = 0; i < idx; i += 1) {
    if (text.charCodeAt(i) === 10) {
      line += 1;
      lastNl = i;
    }
  }
  const col = idx - lastNl;
  return { line, col };
}

/** @param {string} text @param {number} line */
function getLineText(text, line) {
  const lines = text.split(/\r?\n/);
  return lines[line - 1] ?? '';
}

let violations = 0;

for (const abs of walk(ROOT)) {
  const rel = path.relative(ROOT, abs);
  if (isAllowed(rel)) continue;

  const text = fs.readFileSync(abs, 'utf8');

  for (const p of PATTERNS) {
    p.re.lastIndex = 0;
    let m;
    while ((m = p.re.exec(text))) {
      // Ignore token-based rgb(var(--token-rgb) / alpha) strings.
      if (p.name === 'hex' && /%23/i.test(m[0])) continue;

      const { line, col } = lineColFromIndex(text, m.index);
      const lineText = getLineText(text, line);
      violations += 1;
      console.log(`${normalize(rel)}:${line}:${col}\t${p.name}\t${m[0]}`);
      console.log(`  ${lineText}`);
    }
  }
}

if (violations > 0) {
  console.error(`\nFound ${violations} hardcoded color occurrence(s). Use theme tokens from index.css instead.`);
  process.exit(1);
}

console.log('OK: no hardcoded colors detected (outside allowed files).');

