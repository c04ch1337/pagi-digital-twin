# Environment Variables Setup

## Required Environment Variables

Create a `.env.local` file in the `frontend-digital-twin` directory with the following variables:

```env
# OpenRouter API Key (for image generation and LLM access - recommended)
# Get your key from https://openrouter.ai/
VITE_OPENROUTER_API_KEY=sk-or-v1-your-openrouter-api-key-here

# Replicate API Key (for video generation - free tier available)
# Get your free API key from https://replicate.com/account/api-tokens
# Free tier includes credits for testing
VITE_REPLICATE_API_KEY=r8_your-replicate-api-key-here

# Google Gemini API Key (for fallback image generation - optional)
# Only needed if you want Gemini as a fallback for image generation
GEMINI_API_KEY=your_gemini_api_key_here

# PAGI Chat Backend WebSocket URL
# Format: ws://host:port/ws/chat
# The user_id will be appended by the PAGIClient automatically
# Example: ws://127.0.0.1:8181/ws/chat
VITE_WS_URL=ws://127.0.0.1:8181/ws/chat

# Telemetry Server-Sent Events (SSE) URL
# Format: http://host:port/v1/telemetry/stream
# Example: http://127.0.0.1:8181/v1/telemetry/stream
VITE_SSE_URL=http://127.0.0.1:8181/v1/telemetry/stream
```

## Usage

1. Copy this template to `.env.local`:
   ```bash
   cp ENV_SETUP.md .env.local
   # Then edit .env.local and fill in the values
   ```

2. For production, use `wss://` (secure WebSocket):
   ```env
   VITE_WS_URL=wss://your-backend-domain.com/ws/chat
   ```

3. The WebSocket client will automatically append the `user_id` to the URL:
   - Base URL: `ws://127.0.0.1:8181/ws/chat`
   - User ID: `user123`
   - Final URL: `ws://127.0.0.1:8181/ws/chat/user123`

## Vite Environment Variables

Vite automatically loads environment variables prefixed with `VITE_` from `.env.local` files.

Access in code:
```typescript
const wsUrl = import.meta.env.VITE_WS_URL || 'ws://127.0.0.1:8181/ws/chat';
```
