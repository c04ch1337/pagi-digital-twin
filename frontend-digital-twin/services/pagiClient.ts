import { ChatRequest, ChatResponse } from '../types/protocol';

type MessageCallback = (response: ChatResponse) => void;
type ConnectionStateCallback = (connected: boolean) => void;

/**
 * PAGI Chat WebSocket Client
 * 
 * Manages WebSocket connection to the Rust backend and handles
 * the PAGI Chat Protocol message exchange.
 * 
 * The backend expects the user_id in the WebSocket path: /ws/chat/:user_id
 */
export class PAGIClient {
  private ws: WebSocket | null = null;
  private baseUrl: string; // Base URL without user_id (e.g., ws://127.0.0.1:8181/ws/chat)
  private userId: string;
  private messageCallback: MessageCallback;
  private connectionStateCallback?: ConnectionStateCallback;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 3000; // 3 seconds
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private isManualDisconnect = false;

  /**
   * Creates a new PAGI Chat WebSocket client.
   * 
   * @param baseUrl - Base WebSocket URL (e.g., "ws://127.0.0.1:8181/ws/chat")
   * @param userId - User ID to include in the WebSocket path
   * @param messageCallback - Callback function for received messages
   * @param connectionStateCallback - Optional callback for connection state changes
   */
  constructor(
    baseUrl: string,
    userId: string,
    messageCallback: MessageCallback,
    connectionStateCallback?: ConnectionStateCallback
  ) {
    if (!baseUrl) {
      throw new Error('WebSocket URL is required. Set VITE_WS_URL environment variable.');
    }
    if (!userId) {
      throw new Error('User ID is required for WebSocket connection.');
    }
    
    // Ensure baseUrl doesn't end with a slash
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.userId = userId;
    this.messageCallback = messageCallback;
    this.connectionStateCallback = connectionStateCallback;
    this.connect();
  }

  /**
   * Establishes WebSocket connection to the backend.
   * Handles reconnection logic automatically.
   * 
   * The full URL is constructed as: {baseUrl}/{userId}
   * Example: ws://127.0.0.1:8181/ws/chat/user123
   */
  private connect(): void {
    if (this.isManualDisconnect) {
      return; // Don't reconnect if manually disconnected
    }

    const fullUrl = `${this.baseUrl}/${this.userId}`;
    console.log(`[PAGIClient] Attempting to connect to ${fullUrl}`);
    // Do not mark as disconnected before the first connection is attempted;
    // otherwise the UI shows a spurious "connection lost" message during initial connect.

    try {
      this.ws = new WebSocket(fullUrl);

      this.ws.onopen = () => {
        console.log('[PAGIClient] Connection established');
        this.reconnectAttempts = 0; // Reset on successful connection
        this.updateConnectionState(true);
      };

      this.ws.onmessage = (event) => {
        try {
          const response: ChatResponse = JSON.parse(event.data);
          console.debug('[PAGIClient] Received message:', response);
          this.messageCallback(response);
        } catch (error) {
          console.error(
            '[PAGIClient] Failed to parse incoming JSON:',
            event.data,
            error
          );
          // Send a generic error status back to the UI
          this.messageCallback({
            type: 'status_update',
            status: 'error',
            details: 'Failed to parse server response.',
          } as ChatResponse);
        }
      };

      this.ws.onclose = (event) => {
        console.warn(
          `[PAGIClient] Connection closed. Code: ${event.code}, Reason: ${event.reason || 'No reason provided'}`
        );
        this.updateConnectionState(false);

        // Attempt reconnection if not manually disconnected
        if (!this.isManualDisconnect && this.reconnectAttempts < this.maxReconnectAttempts) {
          this.reconnectAttempts++;
          console.log(
            `[PAGIClient] Reconnecting (attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts}) in ${this.reconnectDelay}ms...`
          );
          this.reconnectTimer = setTimeout(() => this.connect(), this.reconnectDelay);
        } else if (this.reconnectAttempts >= this.maxReconnectAttempts) {
          console.error('[PAGIClient] Max reconnection attempts reached. Connection failed.');
          this.messageCallback({
            type: 'status_update',
            status: 'error',
            details: 'Failed to establish connection after multiple attempts. Please refresh the page.',
          } as ChatResponse);
        }
      };

      this.ws.onerror = (error) => {
        console.error('[PAGIClient] WebSocket error:', error);
        // The onclose handler will be called after onerror, which will trigger reconnection
      };
    } catch (error) {
      console.error('[PAGIClient] Failed to create WebSocket:', error);
      this.updateConnectionState(false);
      
      // Attempt reconnection
      if (!this.isManualDisconnect && this.reconnectAttempts < this.maxReconnectAttempts) {
        this.reconnectAttempts++;
        this.reconnectTimer = setTimeout(() => this.connect(), this.reconnectDelay);
      }
    }
  }

  /**
   * Sends a ChatRequest to the backend via WebSocket.
   * 
   * @param request - The chat request to send
   * @returns true if sent successfully, false otherwise
   */
  public sendRequest(request: ChatRequest): boolean {
    if (this.ws?.readyState === WebSocket.OPEN) {
      try {
        const jsonMessage = JSON.stringify(request);
        this.ws.send(jsonMessage);
        console.debug('[PAGIClient] Sent request:', request);
        return true;
      } catch (error) {
        console.error('[PAGIClient] Failed to serialize or send request:', error);
        return false;
      }
    } else {
      const state = this.ws?.readyState;
      const stateName =
        state === WebSocket.CONNECTING
          ? 'CONNECTING'
          : state === WebSocket.CLOSING
          ? 'CLOSING'
          : state === WebSocket.CLOSED
          ? 'CLOSED'
          : 'UNKNOWN';
      console.warn(
        `[PAGIClient] WebSocket not open (state: ${stateName}). Request queued or dropped.`
      );
      
      // Notify UI of connection issue
      this.messageCallback({
        type: 'status_update',
        status: 'error',
        details: 'WebSocket connection not ready. Please wait for connection to establish.',
      } as ChatResponse);
      
      return false;
    }
  }

  /**
   * Manually disconnects from the WebSocket server.
   * Prevents automatic reconnection.
   */
  public disconnect(): void {
    console.log('[PAGIClient] Disconnecting manually...');
    this.isManualDisconnect = true;
    
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    
    this.updateConnectionState(false);
    console.log('[PAGIClient] Disconnected manually.');
  }

  /**
   * Reconnects to the WebSocket server.
   * Resets manual disconnect flag and attempts connection.
   */
  public reconnect(): void {
    console.log('[PAGIClient] Reconnecting...');
    this.isManualDisconnect = false;
    this.reconnectAttempts = 0;
    
    if (this.ws) {
      this.ws.close();
    }
    
    this.connect();
  }

  /**
   * Gets the current connection state.
   * 
   * @returns true if connected and ready, false otherwise
   */
  public isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /**
   * Gets the current WebSocket ready state.
   * 
   * @returns WebSocket ready state (CONNECTING, OPEN, CLOSING, CLOSED)
   */
  public getReadyState(): number | undefined {
    return this.ws?.readyState;
  }

  /**
   * Updates connection state and notifies callback if provided.
   */
  private updateConnectionState(connected: boolean): void {
    if (this.connectionStateCallback) {
      this.connectionStateCallback(connected);
    }
  }
}
