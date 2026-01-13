import { ChatResponse, CompleteMessage, MessageChunk, StatusUpdate } from '../types/protocol';
import { Message } from '../types';

/**
 * Converts ChatResponse from the backend protocol to the UI's Message format
 */
export function convertChatResponseToMessage(
  response: ChatResponse,
  twinId?: string
): Message | null {
  switch (response.type) {
    case 'complete_message':
      return {
        id: response.id,
        sender: 'assistant',
        content: response.content,
        timestamp: new Date(), // Backend doesn't send timestamp, use current time
        twinId,
      };

    case 'message_chunk':
      // For streaming chunks, we'll handle these separately
      // Return null and let the UI handle chunk accumulation
      return null;

    case 'status_update':
      // Option A: hide noisy READY connection banners from the chat stream.
      // Keep error/busy (and any other) status updates visible.
      if (response.status?.toLowerCase() === 'ready') {
        return null;
      }

      // Convert status updates to assistant messages for display
      // Format error messages more clearly
      let content = response.details || 'Status update';
      if (response.status?.toLowerCase() === 'error') {
        // Improve error message formatting
        if (content.includes('Request to orchestrator failed')) {
          content = `Connection Error: Unable to reach the orchestrator service. Please check:\n\n` +
            `1. Is the orchestrator service running? (Check backend-rust-orchestrator)\n` +
            `2. Is the WebSocket URL correct? (Current: ${import.meta.env.VITE_WS_URL || 'ws://127.0.0.1:8181/ws/chat'})\n` +
            `3. Check browser console for detailed connection errors\n\n` +
            `Original error: ${content}`;
        } else if (content.includes('error sending request')) {
          content = `Network Error: Failed to send request to orchestrator.\n\n` +
            `This usually means:\n` +
            `- The orchestrator service is not running\n` +
            `- There's a network connectivity issue\n` +
            `- The service is starting up (wait a few seconds and try again)\n\n` +
            `Check the orchestrator logs for more details.\n\n` +
            `Original error: ${content}`;
        }
      }

      return {
        id: `status-${Date.now()}-${Math.random()}`,
        sender: 'assistant',
        content: `[${response.status.toUpperCase()}] ${content}`,
        timestamp: new Date(),
        twinId,
      };

    default:
      return null;
  }
}

/**
 * Accumulates message chunks into a complete message
 */
export function accumulateChunks(
  chunks: MessageChunk[]
): { id: string; content: string; isFinal: boolean } | null {
  if (chunks.length === 0) return null;

  const firstChunk = chunks[0];
  const content = chunks.map(c => c.content_chunk).join('');
  const isFinal = chunks.some(c => c.is_final);

  return {
    id: firstChunk.id,
    content,
    isFinal,
  };
}
