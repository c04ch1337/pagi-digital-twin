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
      return {
        id: `status-${Date.now()}-${Math.random()}`,
        sender: 'assistant',
        content: `[${response.status.toUpperCase()}] ${response.details || 'Status update'}`,
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
