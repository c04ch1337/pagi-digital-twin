/**
 * Example usage of PAGIClient
 * 
 * This file demonstrates how to integrate the WebSocket client
 * into your React application.
 */

import { PAGIClient } from './pagiClient';
import { ChatRequest, ChatResponse } from '../types/protocol';
import { v4 as uuidv4 } from 'uuid'; // You'll need: npm install uuid @types/uuid

// Example: Initialize the client in a React component
export function initializePAGIClient(userId: string) {
  // Get WebSocket URL from environment variable
  const wsUrl = import.meta.env.VITE_WS_URL || 'ws://127.0.0.1:8181/ws/chat';
  
  // Generate or retrieve session ID (persist this across page reloads)
  const sessionId = localStorage.getItem('pagi_session_id') || uuidv4();
  localStorage.setItem('pagi_session_id', sessionId);

  // Message handler callback
  const handleMessage = (response: ChatResponse) => {
    switch (response.type) {
      case 'complete_message':
        console.log('Complete message received:', response.content);
        console.log('Source memories:', response.source_memories);
        if (response.issued_command) {
          handleAgentCommand(response.issued_command);
        }
        break;
        
      case 'message_chunk':
        console.log('Message chunk:', response.content_chunk);
        // Handle streaming tokens
        break;
        
      case 'status_update':
        console.log('Status update:', response.status, response.details);
        // Update UI with connection status
        break;
    }
  };

  // Connection state callback
  const handleConnectionState = (connected: boolean) => {
    console.log('Connection state:', connected ? 'Connected' : 'Disconnected');
    // Update UI connection indicator
  };

  // Create client instance
  const client = new PAGIClient(
    wsUrl,
    userId,
    handleMessage,
    handleConnectionState
  );

  // Example: Send a message
  const sendMessage = (message: string) => {
    const request: ChatRequest = {
      session_id: sessionId,
      user_id: userId,
      timestamp: new Date().toISOString(),
      message: message,
    };
    
    client.sendRequest(request);
  };

  // Handle agent commands
  const handleAgentCommand = (command: ChatResponse['issued_command']) => {
    if (!command) return;
    
    switch (command.command) {
      case 'show_memory_page':
        // Navigate to memory page
        console.log('Show memory:', command.memory_id, command.query);
        break;
        
      case 'prompt_for_config':
        // Show config prompt modal
        console.log('Config prompt:', command.config_key, command.prompt);
        break;
        
      case 'execute_tool':
        // Execute tool locally
        console.log('Execute tool:', command.tool_name, command.arguments);
        break;
    }
  };

  return {
    client,
    sendMessage,
    sessionId,
  };
}

// Example React hook usage:
/*
import { useEffect, useState, useRef } from 'react';
import { PAGIClient } from './services/pagiClient';
import { ChatRequest, ChatResponse } from './types/protocol';

export function usePAGIClient(userId: string) {
  const [connected, setConnected] = useState(false);
  const [messages, setMessages] = useState<ChatResponse[]>([]);
  const clientRef = useRef<PAGIClient | null>(null);
  const sessionIdRef = useRef<string>(
    localStorage.getItem('pagi_session_id') || crypto.randomUUID()
  );

  useEffect(() => {
    const wsUrl = import.meta.env.VITE_WS_URL || 'ws://127.0.0.1:8181/ws/chat';
    
    const client = new PAGIClient(
      wsUrl,
      userId,
      (response) => {
        setMessages(prev => [...prev, response]);
      },
      (connected) => {
        setConnected(connected);
      }
    );
    
    clientRef.current = client;
    localStorage.setItem('pagi_session_id', sessionIdRef.current);

    return () => {
      client.disconnect();
    };
  }, [userId]);

  const sendMessage = (message: string) => {
    if (!clientRef.current?.isConnected()) {
      console.warn('WebSocket not connected');
      return;
    }

    const request: ChatRequest = {
      session_id: sessionIdRef.current,
      user_id: userId,
      timestamp: new Date().toISOString(),
      message,
    };

    clientRef.current.sendRequest(request);
  };

  return { connected, messages, sendMessage };
}
*/
