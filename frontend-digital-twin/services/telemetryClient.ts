/**
 * Telemetry Client Service
 * 
 * Manages Server-Sent Events (SSE) connection to the backend telemetry stream.
 * Provides real-time system metrics (CPU, RAM, Network, etc.) to the frontend.
 */

type TelemetryCallback = (data: string) => void;
type ConnectionStateCallback = (connected: boolean) => void;

export class TelemetryClient {
  private eventSource: EventSource | null = null;
  private url: string;
  private callback: TelemetryCallback;
  private connectionStateCallback?: ConnectionStateCallback;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10; // SSE has built-in reconnection, but we track attempts
  private isManualDisconnect = false;

  constructor(
    url: string,
    callback: TelemetryCallback,
    connectionStateCallback?: ConnectionStateCallback
  ) {
    if (!url) {
      console.error('[TelemetryClient] Telemetry URL is required. Set VITE_SSE_URL environment variable.');
      return;
    }
    this.url = url;
    this.callback = callback;
    this.connectionStateCallback = connectionStateCallback;
    this.connect();
  }

  private connect(): void {
    if (this.isManualDisconnect) {
      return; // Don't reconnect if manually disconnected
    }

    console.log(`[TelemetryClient] Attempting SSE connection to ${this.url}`);
    // Do not mark as disconnected before the first connection attempt;
    // otherwise the UI shows a spurious "disconnected" state during initial connect.

    try {
      // EventSource is designed for SSE and handles automatic reconnection
      this.eventSource = new EventSource(this.url);

      this.eventSource.onopen = () => {
        console.log('[TelemetryClient] SSE connection established');
        this.reconnectAttempts = 0; // Reset on successful connection
        this.updateConnectionState(true);
      };

      // Listen for generic 'message' events (default SSE event type)
      this.eventSource.onmessage = (event) => {
        try {
          this.callback(event.data);
        } catch (error) {
          console.error('[TelemetryClient] Error processing telemetry data:', error);
        }
      };

      // The Rust telemetry service emits `event: metrics`.
      this.eventSource.addEventListener('metrics', (event) => {
        try {
          const data = (event as MessageEvent).data;
          this.callback(data);
        } catch (error) {
          console.error('[TelemetryClient] Error processing telemetry metrics event:', error);
        }
      });

      this.eventSource.onerror = (error) => {
        console.error('[TelemetryClient] SSE Error:', error);
        this.updateConnectionState(false);
        
        // EventSource attempts to reconnect automatically
        // We just track the attempts for monitoring
        if (this.eventSource?.readyState === EventSource.CLOSED) {
          this.reconnectAttempts++;
          if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('[TelemetryClient] Max reconnection attempts reached. Connection failed.');
            this.updateConnectionState(false);
          }
        }
      };
    } catch (error) {
      console.error('[TelemetryClient] Failed to create EventSource:', error);
      this.updateConnectionState(false);
    }
  }

  /**
   * Manually disconnects from the SSE stream.
   * Prevents automatic reconnection.
   */
  public disconnect(): void {
    console.log('[TelemetryClient] Disconnecting manually...');
    this.isManualDisconnect = true;
    
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
    
    this.updateConnectionState(false);
    console.log('[TelemetryClient] Disconnected manually.');
  }

  /**
   * Reconnects to the SSE stream.
   * Resets manual disconnect flag and attempts connection.
   */
  public reconnect(): void {
    console.log('[TelemetryClient] Reconnecting...');
    this.isManualDisconnect = false;
    this.reconnectAttempts = 0;
    
    if (this.eventSource) {
      this.eventSource.close();
    }
    
    this.connect();
  }

  /**
   * Gets the current connection state.
   * 
   * @returns true if connected, false otherwise
   */
  public isConnected(): boolean {
    return this.eventSource?.readyState === EventSource.OPEN;
  }

  /**
   * Gets the current EventSource ready state.
   * 
   * @returns EventSource ready state (CONNECTING, OPEN, CLOSED)
   */
  public getReadyState(): number | undefined {
    return this.eventSource?.readyState;
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
