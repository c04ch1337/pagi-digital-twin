import path from 'path';
import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, '.', '');
    return {
      server: {
        port: 3000,
        host: '0.0.0.0',
        // Local dev convenience: keep the frontend same-origin while proxying API calls
        // to the Rust Gateway (which then proxies to Telemetry).
        proxy: {
          '/api': {
            target: env.VITE_GATEWAY_HTTP_URL || 'http://127.0.0.1:8181',
            changeOrigin: true,
          },
          '/v1': {
            target: env.VITE_GATEWAY_HTTP_URL || 'http://127.0.0.1:8181',
            changeOrigin: true,
          },
        },
      },
      plugins: [react()],
      define: {
        'process.env.API_KEY': JSON.stringify(env.GEMINI_API_KEY),
        'process.env.GEMINI_API_KEY': JSON.stringify(env.GEMINI_API_KEY),
        'process.env.VITE_OPENROUTER_API_KEY': JSON.stringify(env.VITE_OPENROUTER_API_KEY || env.OPENROUTER_API_KEY),
        'process.env.VITE_REPLICATE_API_KEY': JSON.stringify(env.VITE_REPLICATE_API_KEY || env.REPLICATE_API_KEY),
        'process.env.VITE_WS_URL': JSON.stringify(env.VITE_WS_URL || 'ws://127.0.0.1:8181/ws/chat'),
        'process.env.VITE_SSE_URL': JSON.stringify(env.VITE_SSE_URL || 'http://127.0.0.1:8181/v1/telemetry/stream')
      },
      resolve: {
        alias: {
          '@': path.resolve(__dirname, '.'),
        }
      }
    };
});
