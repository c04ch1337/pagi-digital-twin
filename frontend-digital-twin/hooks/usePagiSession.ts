import { useState, useEffect } from 'react';

/**
 * Custom hook for managing PAGI Chat session state.
 * 
 * Handles:
 * - User ID generation/retrieval (persisted in localStorage)
 * - Session ID generation/retrieval (persisted in localStorage)
 * - Session lifecycle management
 */
export function usePagiSession() {
  const [userId, setUserId] = useState<string>('');
  const [sessionId, setSessionId] = useState<string>('');

  useEffect(() => {
    // Optional override for deterministic E2E testing / policy-controlled tool execution.
    // If set, we force the user/twin id to a known value (e.g., "twin-sentinel").
    const forcedTwinId = (import.meta as any).env?.VITE_FORCE_TWIN_ID as string | undefined;

    // Generate or retrieve user ID (persistent across sessions)
    let storedUserId = localStorage.getItem('pagi_user_id');

    if (forcedTwinId && forcedTwinId.trim().length > 0) {
      storedUserId = forcedTwinId.trim();
      localStorage.setItem('pagi_user_id', storedUserId);
    }

    if (!storedUserId) {
      // Generate a simple user ID (in production, this might come from auth)
      storedUserId = `user_${crypto.randomUUID().replace(/-/g, '').substring(0, 16)}`;
      localStorage.setItem('pagi_user_id', storedUserId);
    }
    setUserId(storedUserId);

    // Generate or retrieve session ID (new session on page load)
    // In a real app, you might want to persist this across page refreshes
    // For now, we generate a new session on each page load
    const storedSessionId = localStorage.getItem('pagi_session_id');
    if (storedSessionId) {
      // Optionally reuse existing session
      setSessionId(storedSessionId);
    } else {
      // Generate new session ID
      const newSessionId = crypto.randomUUID();
      localStorage.setItem('pagi_session_id', newSessionId);
      setSessionId(newSessionId);
    }
  }, []);

  /**
   * Creates a new session (useful for "New Chat" functionality)
   */
  const createNewSession = () => {
    const newSessionId = crypto.randomUUID();
    localStorage.setItem('pagi_session_id', newSessionId);
    setSessionId(newSessionId);
  };

  return {
    userId,
    sessionId,
    createNewSession,
  };
}
