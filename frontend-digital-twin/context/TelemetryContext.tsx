import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { TelemetryData } from '../types';
import { TelemetryClient } from '../services/telemetryClient';

// --- Define Context State ---
interface TelemetryContextType {
  telemetry: TelemetryData[];
  isConnected: boolean;
  latestData: TelemetryData | null;
}

const TelemetryContext = createContext<TelemetryContextType | undefined>(undefined);

// --- Provider Component ---
export const TelemetryProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [telemetry, setTelemetry] = useState<TelemetryData[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const clientRef = useRef<TelemetryClient | null>(null);
  const maxDataPoints = 100; // Keep last 100 data points

  const handleTelemetryData = useCallback((data: string) => {
    try {
      // Parse the JSON telemetry data from SSE
      const parsed = JSON.parse(data);

      // backend-rust-telemetry emits:
      // { ts_ms, cpu_percent, mem_total, mem_used, mem_free, process_count }
      const cpu = typeof parsed.cpu_percent === 'number' ? parsed.cpu_percent : (parsed.cpu ?? 0);
      const memTotal = typeof parsed.mem_total === 'number' ? parsed.mem_total : 0;
      const memUsed = typeof parsed.mem_used === 'number' ? parsed.mem_used : 0;
      const memPercent = memTotal > 0 ? (memUsed / memTotal) * 100 : (parsed.memory ?? 0);

      const telemetryPoint: TelemetryData = {
        cpu,
        memory: memPercent,
        gpu: 0,
        network: 0,
        timestamp: new Date().toLocaleTimeString(),
      };

      setTelemetry(prev => {
        // Add new data point and keep only the last maxDataPoints
        const updated = [...prev, telemetryPoint].slice(-maxDataPoints);
        return updated;
      });
    } catch (error) {
      console.error('[TelemetryContext] Failed to parse telemetry data:', data, error);
    }
  }, []);

  const handleConnectionState = useCallback((connected: boolean) => {
    setIsConnected(connected);
    if (connected) {
      console.log('[TelemetryContext] Telemetry stream connected');
    } else {
      console.warn('[TelemetryContext] Telemetry stream disconnected');
    }
  }, []);

  // Initialize Telemetry client when component mounts
  useEffect(() => {
    const sseUrl = import.meta.env.VITE_SSE_URL || 'http://127.0.0.1:8181/v1/telemetry/stream';
    
    console.log('[TelemetryContext] Initializing TelemetryClient');
    const client = new TelemetryClient(
      sseUrl,
      handleTelemetryData,
      handleConnectionState
    );
    
    clientRef.current = client;

    // Cleanup function for disconnect
    return () => {
      if (clientRef.current) {
        console.log('[TelemetryContext] Cleaning up TelemetryClient');
        clientRef.current.disconnect();
        clientRef.current = null;
      }
    };
  }, [handleTelemetryData, handleConnectionState]);

  // Get latest telemetry data point
  const latestData = useMemo(() => {
    return telemetry.length > 0 ? telemetry[telemetry.length - 1] : null;
  }, [telemetry]);

  const value = useMemo(() => ({
    telemetry,
    isConnected,
    latestData,
  }), [telemetry, isConnected, latestData]);

  return (
    <TelemetryContext.Provider value={value}>
      {children}
    </TelemetryContext.Provider>
  );
};

// --- Custom Hook for Consumption ---
export const useTelemetry = (): TelemetryContextType => {
  const context = useContext(TelemetryContext);
  if (context === undefined) {
    throw new Error('useTelemetry must be used within a TelemetryProvider');
  }
  return context;
};
