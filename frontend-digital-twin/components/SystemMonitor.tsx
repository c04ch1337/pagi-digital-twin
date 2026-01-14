import React, { useState, useEffect, useCallback } from 'react';
import { fetchSystemSnapshot, SystemSnapshot, ProcessSnapshot } from '../services/systemService';
import { usePagi } from '../context/PagiContext';
import NetworkMap from './NetworkMap';
import { fetchLatestNetworkScan, runNetworkScan } from '../services/networkScanService';
import type { NetworkScanResult } from '../types/networkScan';
import { formatCompactKiB } from '../utils/formatMetrics';

interface SystemMonitorProps {
  onClose?: () => void;
  twinId: string;
}

const SystemMonitor: React.FC<SystemMonitorProps> = ({ onClose, twinId }) => {
  const [snapshot, setSnapshot] = useState<SystemSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { sendChatRequest } = usePagi();

  const [scanTarget, setScanTarget] = useState<string>('192.168.1.0/24');
  const [showScanAdvanced, setShowScanAdvanced] = useState<boolean>(false);
  const [publicScanToken, setPublicScanToken] = useState<string>('');
  const [scan, setScan] = useState<NetworkScanResult | null>(null);
  const [scanLoading, setScanLoading] = useState<boolean>(false);
  const [scanError, setScanError] = useState<string | null>(null);

  const isLikelyPrivateTarget = (t: string): boolean => {
    const s = (t || '').trim();
    const ip = s.split('/')[0];
    const parts = ip.split('.');
    if (parts.length !== 4) return false;
    const a = Number(parts[0]);
    const b = Number(parts[1]);
    if (!Number.isFinite(a) || !Number.isFinite(b)) return false;
    if (a === 10) return true;
    if (a === 172 && b >= 16 && b <= 31) return true;
    if (a === 192 && b === 168) return true;
    if (a === 169 && b === 254) return true;
    return false;
  };

  const loadSnapshot = useCallback(async () => {
    try {
      setError(null);
      const data = await fetchSystemSnapshot();
      setSnapshot(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load system snapshot');
      console.error('Failed to fetch system snapshot:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSnapshot();
    // Poll every 2 seconds
    const interval = setInterval(loadSnapshot, 2000);
    return () => clearInterval(interval);
  }, [loadSnapshot]);

  const loadLatestScan = useCallback(async () => {
    try {
      setScanError(null);
      const latest = await fetchLatestNetworkScan({ twin_id: twinId, namespace: 'default' });
      setScan(latest);
    } catch (e) {
      setScanError(e instanceof Error ? e.message : 'Failed to load network scan');
    }
  }, [twinId]);

  useEffect(() => {
    loadLatestScan();
  }, [loadLatestScan]);

  const handleRescanNetwork = async () => {
    if (!scanTarget.trim()) return;
    setScanLoading(true);
    setScanError(null);
    try {
      const token = publicScanToken.trim();
      const res = await runNetworkScan({
        target: scanTarget.trim(),
        twin_id: twinId,
        namespace: 'default',
        hitl_token: token.length > 0 ? token : undefined,
      });
      setScan(res);
    } catch (e) {
      setScanError(e instanceof Error ? e.message : 'Network scan failed');
    } finally {
      setScanLoading(false);
    }
  };

  const handleAskAIOptimize = () => {
    if (!snapshot) return;

    const ramPercent = (snapshot.memory.used_kib / snapshot.memory.total_kib) * 100;
    const cpuPercent = snapshot.cpu.global_usage_percent;
    
    const prompt = `My system is at ${ramPercent.toFixed(1)}% RAM usage and ${cpuPercent.toFixed(1)}% CPU usage. Based on the current process list, what should I close to improve performance?`;
    
    if (sendChatRequest) {
      sendChatRequest(prompt);
      // Optionally close the monitor and navigate to chat
      if (onClose) {
        onClose();
      }
    }
  };

  const handleTerminateProcess = (pid: number, name: string) => {
    if (!sendChatRequest) return;
    
    const prompt = `I want to terminate process "${name}" (PID: ${pid}). Please request permission to kill this process.`;
    sendChatRequest(prompt);
    
    // Optionally close the monitor and navigate to chat
    if (onClose) {
      onClose();
    }
  };

  const getUsageColor = (percent: number): string => {
    if (percent < 50) return 'rgb(var(--success-rgb))';
    if (percent < 75) return 'rgb(var(--info-rgb))';
    if (percent < 90) return 'rgb(var(--warning-rgb))';
    return 'rgb(var(--danger-rgb))';
  };

  if (loading && !snapshot) {
    return (
      <div className="flex-1 flex items-center justify-center bg-[var(--bg-primary)]">
        <div className="text-center">
          <div className="text-sm text-[var(--text-secondary)] mb-2">Loading system snapshot...</div>
          <div className="w-8 h-8 border-4 border-[var(--bg-steel)] border-t-transparent rounded-full animate-spin mx-auto"></div>
        </div>
      </div>
    );
  }

  if (error && !snapshot) {
    return (
      <div className="flex-1 flex items-center justify-center bg-[var(--bg-primary)]">
        <div className="text-center p-4 bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg">
          <div className="text-sm text-[var(--danger)] mb-2">Error loading system snapshot</div>
          <div className="text-xs text-[var(--text-secondary)] mb-4">{error}</div>
          <button
            onClick={loadSnapshot}
            className="px-4 py-2 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (!snapshot) return null;

  const ramPercent = (snapshot.memory.used_kib / snapshot.memory.total_kib) * 100;
  const cpuPercent = snapshot.cpu.global_usage_percent;

  return (
    <div className="flex-1 flex flex-col bg-[var(--bg-primary)] overflow-hidden">
      {/* Header */}
      <div className="p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)] flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">monitor</span>
          <h2 className="text-sm font-bold text-[var(--text-secondary)]">System Status</h2>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleAskAIOptimize}
            className="px-3 py-1.5 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all flex items-center gap-1"
          >
            <span className="material-symbols-outlined text-sm">auto_awesome</span>
            Ask AI to Optimize
          </button>
          {onClose && (
            <button
              onClick={onClose}
              className="p-1.5 hover:bg-[var(--bg-muted)] rounded-md transition-colors"
            >
              <span className="material-symbols-outlined text-[var(--text-secondary)] text-lg">close</span>
            </button>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-6">
        {/* Network Map */}
        <div className="bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4">
          <div className="flex items-center justify-between gap-2 mb-3">
            <div className="flex items-center gap-2">
              <span className="material-symbols-outlined text-[var(--bg-steel)]">hub</span>
              <h3 className="text-xs font-bold text-[var(--text-secondary)] uppercase tracking-wider">Network Scanner</h3>
            </div>
            <button
              onClick={handleRescanNetwork}
              disabled={scanLoading || !scanTarget.trim()}
              className="px-3 py-1.5 bg-[var(--bg-steel)] hover:bg-[var(--bg-muted)] disabled:opacity-50 rounded-lg text-xs font-bold text-[var(--text-on-accent)] transition-all flex items-center gap-1"
            >
              <span className="material-symbols-outlined text-sm">refresh</span>
              Rescan Network
            </button>
          </div>

          <div className="flex flex-col md:flex-row gap-2 mb-4">
            <input
              value={scanTarget}
              onChange={(e) => setScanTarget(e.target.value)}
              placeholder="192.168.1.0/24"
              className="flex-1 bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg px-3 py-2 text-xs font-mono focus:ring-1 focus:ring-[rgb(var(--bg-steel-rgb)/0.4)]"
            />
            <button
              type="button"
              onClick={() => {
                // Quick presets for common lab subnets
                if (!scanTarget.trim()) setScanTarget('192.168.1.0/24');
              }}
              className="px-3 py-2 bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-xs font-bold text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/1)] transition-all"
              title="Preset"
            >
              Preset
            </button>

            <button
              type="button"
              onClick={() => setShowScanAdvanced(v => !v)}
              className="px-3 py-2 bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-xs font-bold text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/1)] transition-all"
              title="Advanced (public scan HITL token)"
            >
              {showScanAdvanced ? 'Hide Advanced' : 'Advanced'}
            </button>
          </div>

          {!isLikelyPrivateTarget(scanTarget) && (
            <div className="mb-3 text-[11px] text-[rgb(var(--warning-rgb)/0.98)] bg-[rgb(var(--warning-rgb)/0.15)] border border-[rgb(var(--warning-rgb)/0.3)] rounded-lg px-3 py-2">
              Public/non-RFC1918 target detected. This will be blocked unless the server enables public scans
              (<span className="font-mono">ALLOW_PUBLIC_NETWORK_SCAN=1</span>) and you provide a valid HITL token
              (<span className="font-mono">NETWORK_SCAN_HITL_TOKEN</span>).
            </div>
          )}

          {showScanAdvanced && (
            <div className="mb-4 grid grid-cols-1 md:grid-cols-2 gap-2">
              <div>
                <label className="block text-[10px] font-bold text-[var(--text-secondary)] uppercase tracking-widest mb-1">
                  Public Scan HITL Token (optional)
                </label>
                <input
                  value={publicScanToken}
                  onChange={(e) => setPublicScanToken(e.target.value)}
                  placeholder="(matches NETWORK_SCAN_HITL_TOKEN)"
                  className="w-full bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg px-3 py-2 text-xs font-mono focus:ring-1 focus:ring-[rgb(var(--bg-steel-rgb)/0.4)]"
                />
              </div>
              <div className="text-[11px] text-[var(--text-secondary)] opacity-80 leading-relaxed">
                Use this only when explicitly authorized. Planner-triggered scans remain internal-only.
              </div>
            </div>
          )}

          <NetworkMap scan={scan} loading={scanLoading} error={scanError} />
        </div>
        {/* CPU and RAM Gauges */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* CPU Gauge */}
          <div className="bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-xs font-bold text-[var(--text-secondary)] uppercase tracking-wider">CPU Usage</h3>
              <span className="text-lg font-bold" style={{ color: getUsageColor(cpuPercent) }}>
                {cpuPercent.toFixed(1)}%
              </span>
            </div>
            <div className="relative h-32">
              {/* Circular Progress */}
              <svg className="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
                <circle
                  cx="50"
                  cy="50"
                  r="40"
                  fill="none"
                  stroke="rgb(var(--bg-steel-rgb)/0.25)"
                  strokeWidth="8"
                />
                <circle
                  cx="50"
                  cy="50"
                  r="40"
                  fill="none"
                  stroke={getUsageColor(cpuPercent)}
                  strokeWidth="8"
                  strokeDasharray={`${2 * Math.PI * 40}`}
                  strokeDashoffset={`${2 * Math.PI * 40 * (1 - cpuPercent / 100)}`}
                  strokeLinecap="round"
                  className="transition-all duration-300"
                />
              </svg>
              <div className="absolute inset-0 flex items-center justify-center">
                <div className="text-center">
                  <div className="text-2xl font-bold" style={{ color: getUsageColor(cpuPercent) }}>
                    {cpuPercent.toFixed(1)}%
                  </div>
                  <div className="text-[9px] text-[var(--text-secondary)] opacity-60 mt-1">
                    {snapshot.cpu.per_core_usage_percent.length} cores
                  </div>
                </div>
              </div>
            </div>
            {snapshot.cpu.per_core_usage_percent.length > 0 && (
              <div className="mt-2 text-[9px] text-[var(--text-secondary)] opacity-70">
                Per-core: {snapshot.cpu.per_core_usage_percent.map((p, i) => `${i}:${p.toFixed(0)}%`).join(', ')}
              </div>
            )}
          </div>

          {/* RAM Gauge */}
          <div className="bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-xs font-bold text-[var(--text-secondary)] uppercase tracking-wider">Memory Usage</h3>
              <span className="text-lg font-bold" style={{ color: getUsageColor(ramPercent) }}>
                {ramPercent.toFixed(1)}%
              </span>
            </div>
            <div className="relative h-32">
              {/* Circular Progress */}
              <svg className="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
                <circle
                  cx="50"
                  cy="50"
                  r="40"
                  fill="none"
                  stroke="rgb(var(--bg-steel-rgb)/0.25)"
                  strokeWidth="8"
                />
                <circle
                  cx="50"
                  cy="50"
                  r="40"
                  fill="none"
                  stroke={getUsageColor(ramPercent)}
                  strokeWidth="8"
                  strokeDasharray={`${2 * Math.PI * 40}`}
                  strokeDashoffset={`${2 * Math.PI * 40 * (1 - ramPercent / 100)}`}
                  strokeLinecap="round"
                  className="transition-all duration-300"
                />
              </svg>
              <div className="absolute inset-0 flex items-center justify-center">
                <div className="text-center">
                  <div className="text-2xl font-bold" style={{ color: getUsageColor(ramPercent) }}>
                    {ramPercent.toFixed(1)}%
                  </div>
                </div>
              </div>
            </div>

            {/* Keep the raw numbers OUTSIDE the circular gauge to prevent overlap on smaller widths */}
            <div className="mt-2 text-[10px] text-[var(--text-secondary)] opacity-70 text-center leading-tight break-words">
              {formatCompactKiB(snapshot.memory.used_kib)} / {formatCompactKiB(snapshot.memory.total_kib)}
            </div>
          </div>
        </div>

        {/* Top Processes Table */}
        <div className="bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg overflow-hidden">
          <div className="p-3 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
            <h3 className="text-xs font-bold text-[var(--text-secondary)] uppercase tracking-wider">
              Top 10 Processes by Memory
            </h3>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead className="bg-[rgb(var(--bg-muted-rgb)/0.3)]">
                <tr>
                  <th className="text-left p-2 text-[var(--text-secondary)] font-bold">Process</th>
                  <th className="text-left p-2 text-[var(--text-secondary)] font-bold">PID</th>
                  <th className="text-right p-2 text-[var(--text-secondary)] font-bold">Memory</th>
                  <th className="text-center p-2 text-[var(--text-secondary)] font-bold">Action</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.top_processes.map((process, index) => (
                  <tr
                    key={`${process.pid}-${index}`}
                    className="border-b border-[rgb(var(--bg-steel-rgb)/0.2)] hover:bg-[rgb(var(--bg-secondary-rgb)/0.2)] transition-colors"
                  >
                    <td className="p-2 text-[var(--text-secondary)] font-mono text-[10px]">{process.name}</td>
                    <td className="p-2 text-[var(--text-secondary)] font-mono text-[10px]">{process.pid}</td>
                    <td className="p-2 text-right text-[var(--text-secondary)] font-mono text-[10px]">
                      {formatCompactKiB(process.memory_kib)}
                    </td>
                    <td className="p-2 text-center">
                      <button
                        onClick={() => handleTerminateProcess(process.pid, process.name)}
                        className="px-2 py-1 bg-[rgb(var(--danger-rgb)/0.85)] hover:bg-[rgb(var(--danger-rgb)/0.95)] text-[var(--text-on-accent)] rounded text-[9px] font-bold transition-all"
                        title={`Terminate ${process.name} (PID: ${process.pid})`}
                      >
                        Terminate
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SystemMonitor;
