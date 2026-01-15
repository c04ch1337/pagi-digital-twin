import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { PAGIClient } from '../services/pagiClient';
import { ChatRequest, ChatResponse, CompleteMessage, MessageChunk, StatusUpdate } from '../types/protocol';
import { usePagiSession } from '../hooks/usePagiSession';
import { getUserName } from '../utils/userName';
import { useDomainAttribution } from './DomainAttributionContext';

// --- Define Context State ---
interface PagiContextType {
  client: PAGIClient | null;
  isConnected: boolean;
  messages: ChatResponse[];
  sendChatRequest: (message: string, settings?: { temperature?: number; top_p?: number; max_tokens?: number; max_memory?: number }) => void;
  currentUserId: string;
  sessionId: string;
  createNewSession: () => string;
  switchToSession: (sessionId: string) => void;
}

const PagiContext = createContext<PagiContextType | undefined>(undefined);

// --- Provider Component ---
export const PagiProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { userId, sessionId, createNewSession, switchToSession } = usePagiSession();
  const { updateAttribution, clearAttribution } = useDomainAttribution();
  const [client, setClient] = useState<PAGIClient | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [messages, setMessages] = useState<ChatResponse[]>([]);
  const clientRef = useRef<PAGIClient | null>(null);

  const handleIncomingMessage = useCallback((response: ChatResponse) => {
    setMessages(prev => {
      // Handle streaming chunks: if message ID exists, append to existing content
      if (response.type === 'message_chunk') {
        const chunkMsg = response as MessageChunk;
        const existingIndex = prev.findIndex(m => m.type === 'message_chunk' && (m as MessageChunk).id === chunkMsg.id);
        if (existingIndex >= 0) {
          // Update existing chunk message
          const existing = prev[existingIndex] as MessageChunk;
          const updated: MessageChunk = {
            ...existing,
            content_chunk: existing.content_chunk + chunkMsg.content_chunk,
            is_final: chunkMsg.is_final,
          };
          const newMessages = [...prev];
          newMessages[existingIndex] = updated;
          return newMessages;
        }
      }
      
      // Extract domain attribution from complete messages
      if (response.type === 'complete_message') {
        const completeMsg = response as CompleteMessage;
        if (completeMsg.domain_attribution) {
          updateAttribution(completeMsg.domain_attribution, completeMsg.id);
        }
      }
      
      // For complete messages or new chunks, append
      return [...prev, response];
    });
  }, [updateAttribution]);

  const handleConnectionStatus = useCallback((connected: boolean) => {
    setIsConnected(connected);
    if (connected) {
      // Clear stale disconnect/error status messages from previous reconnect attempts.
      setMessages(prev =>
        prev.filter(m => !(m.type === 'status_update' && m.status === 'error'))
      );

      // Send a 'Connection Established' status to chat
      const statusMsg: StatusUpdate = {
        type: 'status_update',
        status: 'ready',
        details: 'Connection to Digital Twin established.',
      };
      setMessages(prev => [...prev, statusMsg]);
    } else {
      // Send a disconnection status
      const statusMsg: StatusUpdate = {
        type: 'status_update',
        status: 'error',
        details: 'Connection lost. Attempting to reconnect...',
      };
      setMessages(prev => [...prev, statusMsg]);
    }
  }, []);

  // Initialize WebSocket client when userId and sessionId are available
  useEffect(() => {
    if (!userId || !sessionId) {
      return;
    }

    // If client already exists and is for the same user, don't recreate
    if (clientRef.current) {
      return;
    }

    const wsUrl = import.meta.env.VITE_WS_URL || 'ws://127.0.0.1:8181/ws/chat';
    console.log('[PagiContext] Initializing PAGIClient for user:', userId, 'session:', sessionId);
    
    const newClient = new PAGIClient(
      wsUrl,
      userId,
      handleIncomingMessage,
      handleConnectionStatus
    );
    
    clientRef.current = newClient;
    setClient(newClient);

    // Cleanup function for disconnect
    return () => {
      if (clientRef.current) {
        console.log('[PagiContext] Cleaning up PAGIClient');
        clientRef.current.disconnect();
        clientRef.current = null;
        setClient(null);
      }
    };
  }, [userId, sessionId, handleIncomingMessage, handleConnectionStatus]);

  // Reconnect when session changes (for new chat sessions)
  // Note: first render uses an empty sessionId, so we must seed the ref once sessionId is available
  // to avoid an immediate "session changed" disconnect/reconnect loop.
  const prevSessionIdRef = React.useRef<string>(sessionId);
  useEffect(() => {
    if (!sessionId) {
      return;
    }

    // Seed on first non-empty sessionId
    if (!prevSessionIdRef.current) {
      prevSessionIdRef.current = sessionId;
      return;
    }

    if (prevSessionIdRef.current !== sessionId && clientRef.current && userId) {
      console.log('[PagiContext] Session changed, reconnecting...');

      // Clear the in-memory transcript so the UI starts clean for the new session.
      setMessages([]);
      clearAttribution();

      // Disconnect old client
      clientRef.current.disconnect();
      clientRef.current = null;
      setClient(null);
      
      // Create new client for new session
      const wsUrl = import.meta.env.VITE_WS_URL || 'ws://127.0.0.1:8181/ws/chat';
      const newClient = new PAGIClient(
        wsUrl,
        userId,
        handleIncomingMessage,
        handleConnectionStatus
      );
      
      clientRef.current = newClient;
      setClient(newClient);
      prevSessionIdRef.current = sessionId;
    }
  }, [sessionId, userId, handleIncomingMessage, handleConnectionStatus]);

  const sendChatRequest = useCallback((message: string, settings?: { temperature?: number; top_p?: number; max_tokens?: number; max_memory?: number }) => {
    if (clientRef.current && isConnected && sessionId) {
      const mediaActive = localStorage.getItem('pagi_media_active') === 'true';
      const userName = getUserName(); // Get user name from localStorage
      const request: ChatRequest = {
        session_id: sessionId,
        user_id: userId,
        timestamp: new Date().toISOString(),
        message: message,
        media_active: mediaActive,
        user_name: userName !== 'FG_User' ? userName : undefined, // Only send if not default
        temperature: settings?.temperature,
        top_p: settings?.top_p,
        max_tokens: settings?.max_tokens,
        max_memory: settings?.max_memory,
      };
      
      const sent = clientRef.current.sendRequest(request);
      if (sent) {
        console.log('[PagiContext] Sent chat request:', message, 'from user:', userName);
        // Note: We don't add user message to messages here because
        // the backend will echo it back or we can add it in the UI component
      } else {
        console.error('[PagiContext] Failed to send chat request');
      }
    } else {
      console.warn('[PagiContext] Cannot send message: Client not connected or session not ready');
      const statusMsg: StatusUpdate = {
        type: 'status_update',
        status: 'error',
        details: 'Cannot send message: Connection not ready. Please wait...',
      };
      setMessages(prev => [...prev, statusMsg]);
    }
  }, [isConnected, userId, sessionId]);

  const value = useMemo(() => ({
    client: clientRef.current,
    isConnected,
    messages,
    sendChatRequest,
    currentUserId: userId,
    sessionId,
    createNewSession,
    switchToSession,
  }), [isConnected, messages, sendChatRequest, userId, sessionId, createNewSession, switchToSession]);

  return (
    <PagiContext.Provider value={value}>
      {children}
    </PagiContext.Provider>
  );
};

// --- Custom Hook for Consumption ---
export const usePagi = (): PagiContextType => {
  const context = useContext(PagiContext);
  if (context === undefined) {
    throw new Error('usePagi must be used within a PagiProvider');
  }
  return context;
};
