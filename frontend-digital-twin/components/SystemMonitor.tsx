import React, { useState, useEffect, useCallback } from 'react';
import { fetchSystemSnapshot, SystemSnapshot, ProcessSnapshot } from '../services/systemService';
import { usePagi } from '../context/PagiContext';

interface SystemMonitorProps {
  onClose?: () => void;
}

const SystemMonitor: React.FC<SystemMonitorProps> = ({ onClose }) => {
  const [snapshot, setSnapshot] = useState<SystemSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { sendChatRequest } = usePagi();

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

  const formatBytes = (kib: number): string => {
    const mib = kib / 1024;
    if (mib < 1024) {
      return `${mib.toFixed(1)} MiB`;
    }
    const gib = mib / 1024;
    return `${gib.toFixed(2)} GiB`;
  };

  const getUsageColor = (percent: number): string => {
    if (percent < 50) return '#78A2C2'; // Blue-green
    if (percent < 75) return '#90C3EA'; // Light blue
    if (percent < 90) return '#FFA500'; // Orange
    return '#FF4444'; // Red
  };

  if (loading && !snapshot) {
    return (
      <div className="flex-1 flex items-center justify-center bg-[#9EC9D9]">
        <div className="text-center">
          <div className="text-sm text-[#163247] mb-2">Loading system snapshot...</div>
          <div className="w-8 h-8 border-4 border-[#5381A5] border-t-transparent rounded-full animate-spin mx-auto"></div>
        </div>
      </div>
    );
  }

  if (error && !snapshot) {
    return (
      <div className="flex-1 flex items-center justify-center bg-[#9EC9D9]">
        <div className="text-center p-4 bg-white/70 border border-[#5381A5]/30 rounded-lg">
          <div className="text-sm text-red-600 mb-2">Error loading system snapshot</div>
          <div className="text-xs text-[#163247] mb-4">{error}</div>
          <button
            onClick={loadSnapshot}
            className="px-4 py-2 bg-[#5381A5] hover:bg-[#78A2C2] rounded-lg text-xs font-bold text-white transition-all"
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
    <div className="flex-1 flex flex-col bg-[#9EC9D9] overflow-hidden">
      {/* Header */}
      <div className="p-4 border-b border-[#5381A5]/30 bg-[#90C3EA] flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[#5381A5]">monitor</span>
          <h2 className="text-sm font-bold text-[#163247]">System Status</h2>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleAskAIOptimize}
            className="px-3 py-1.5 bg-[#5381A5] hover:bg-[#78A2C2] rounded-lg text-xs font-bold text-white transition-all flex items-center gap-1"
          >
            <span className="material-symbols-outlined text-sm">auto_awesome</span>
            Ask AI to Optimize
          </button>
          {onClose && (
            <button
              onClick={onClose}
              className="p-1.5 hover:bg-[#78A2C2] rounded-md transition-colors"
            >
              <span className="material-symbols-outlined text-[#163247] text-lg">close</span>
            </button>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-6">
        {/* CPU and RAM Gauges */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* CPU Gauge */}
          <div className="bg-white/70 border border-[#5381A5]/30 rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-xs font-bold text-[#163247] uppercase tracking-wider">CPU Usage</h3>
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
                  stroke="#E5E7EB"
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
                  <div className="text-[9px] text-[#163247] opacity-60 mt-1">
                    {snapshot.cpu.per_core_usage_percent.length} cores
                  </div>
                </div>
              </div>
            </div>
            {snapshot.cpu.per_core_usage_percent.length > 0 && (
              <div className="mt-2 text-[9px] text-[#163247] opacity-70">
                Per-core: {snapshot.cpu.per_core_usage_percent.map((p, i) => `${i}:${p.toFixed(0)}%`).join(', ')}
              </div>
            )}
          </div>

          {/* RAM Gauge */}
          <div className="bg-white/70 border border-[#5381A5]/30 rounded-lg p-4">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-xs font-bold text-[#163247] uppercase tracking-wider">Memory Usage</h3>
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
                  stroke="#E5E7EB"
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
                  <div className="text-[9px] text-[#163247] opacity-60 mt-1">
                    {formatBytes(snapshot.memory.used_kib)} / {formatBytes(snapshot.memory.total_kib)}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Top Processes Table */}
        <div className="bg-white/70 border border-[#5381A5]/30 rounded-lg overflow-hidden">
          <div className="p-3 border-b border-[#5381A5]/30 bg-[#90C3EA]">
            <h3 className="text-xs font-bold text-[#163247] uppercase tracking-wider">
              Top 10 Processes by Memory
            </h3>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead className="bg-[#78A2C2]/30">
                <tr>
                  <th className="text-left p-2 text-[#163247] font-bold">Process</th>
                  <th className="text-left p-2 text-[#163247] font-bold">PID</th>
                  <th className="text-right p-2 text-[#163247] font-bold">Memory</th>
                  <th className="text-center p-2 text-[#163247] font-bold">Action</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.top_processes.map((process, index) => (
                  <tr
                    key={`${process.pid}-${index}`}
                    className="border-b border-[#5381A5]/20 hover:bg-[#90C3EA]/20 transition-colors"
                  >
                    <td className="p-2 text-[#163247] font-mono text-[10px]">{process.name}</td>
                    <td className="p-2 text-[#163247] font-mono text-[10px]">{process.pid}</td>
                    <td className="p-2 text-right text-[#163247] font-mono text-[10px]">
                      {formatBytes(process.memory_kib)}
                    </td>
                    <td className="p-2 text-center">
                      <button
                        onClick={() => handleTerminateProcess(process.pid, process.name)}
                        className="px-2 py-1 bg-red-500/80 hover:bg-red-600 text-white rounded text-[9px] font-bold transition-all"
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
