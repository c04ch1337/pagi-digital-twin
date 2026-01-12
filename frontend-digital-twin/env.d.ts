/// <reference types="vite/client" />

// Optional: declare known env vars for better typing.
interface ImportMetaEnv {
  readonly VITE_WS_URL?: string;
  readonly VITE_SSE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

