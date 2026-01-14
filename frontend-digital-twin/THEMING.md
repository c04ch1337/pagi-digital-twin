# Theming (Light/Dark Mode)

This frontend uses **CSS variables (tokens)** defined in [`index.css`](frontend-digital-twin/index.css:1) and applies them via **Tailwind arbitrary values**.

## How it works

- Light theme variables live under `:root` in [`index.css`](frontend-digital-twin/index.css:10).
- Dark theme overrides live under `[data-theme="dark"]` in [`index.css`](frontend-digital-twin/index.css:58).
- Components should **never** hardcode colors (hex, `rgb()`, named colors) unless the file is explicitly allowed (see guardrail below).

## Canonical tokens

Use these variables in Tailwind arbitrary values:

- Backgrounds: `var(--bg-primary)`, `var(--bg-secondary)`, `var(--bg-muted)`, `var(--bg-steel)`
- Text: `var(--text-primary)`, `var(--text-secondary)`, `var(--text-muted)`
- Borders: `rgb(var(--bg-steel-rgb) / <alpha>)` or `var(--border-color)`
- Accent: `var(--accent)`, hover: `var(--accent-hover)`, text on accent: `var(--text-on-accent)`
- Status: `var(--danger)`, `var(--warning)`, `var(--success)`, `var(--info)` (+ `--*-rgb` companions)

### Examples

- `className="bg-[var(--bg-secondary)] text-[var(--text-primary)]"`
- `className="border border-[rgb(var(--bg-steel-rgb)/0.3)]"`
- `className="bg-[rgb(var(--danger-rgb)/0.12)] text-[rgb(var(--danger-rgb)/0.95)]"`

## Guardrail (prevent regressions)

Run:

```bash
cd frontend-digital-twin
npm run theme:check
```

This executes [`check-theme-colors.mjs`](frontend-digital-twin/scripts/check-theme-colors.mjs:1) which fails if hardcoded colors are found outside:

- [`index.css`](frontend-digital-twin/index.css:1) (token definitions)
- `public/**/*.svg` (brand assets)
- [`refactor-theme-colors.mjs`](frontend-digital-twin/scripts/refactor-theme-colors.mjs:1) (codemod)

