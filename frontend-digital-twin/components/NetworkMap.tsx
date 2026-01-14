import React from 'react';
import type { NetworkScanResult, NetworkScanHost } from '../types/networkScan';

interface NetworkMapProps {
  scan: NetworkScanResult | null;
  loading?: boolean;
  error?: string | null;
}

const HostCard: React.FC<{ host: NetworkScanHost }> = ({ host }) => {
  const title = host.ipv4 || host.hostnames[0] || 'Unknown host';
  const ports = host.ports || [];

  return (
    <div
      className={`bg-[rgb(var(--surface-rgb)/0.7)] border rounded-xl p-3 transition-colors ${
        host.is_agi_core_node ? 'border-[var(--bg-steel)] ring-1 ring-[rgb(var(--bg-steel-rgb)/0.3)]' : 'border-[rgb(var(--bg-steel-rgb)/0.3)]'
      }`}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="text-[11px] font-bold text-[var(--text-primary)] truncate" title={title}>
            {title}
          </div>
          {host.hostnames.length > 0 && (
            <div className="text-[10px] text-[var(--text-secondary)] opacity-80 truncate" title={host.hostnames.join(', ')}>
              {host.hostnames.join(', ')}
            </div>
          )}
        </div>
        {host.is_agi_core_node && (
          <div className="shrink-0 px-2 py-1 rounded-lg bg-[var(--bg-steel)] text-[var(--text-on-accent)] text-[9px] font-black uppercase tracking-wider">
            AGI Core
          </div>
        )}
      </div>

      <div className="mt-2">
        <div className="text-[9px] font-bold text-[var(--text-secondary)] uppercase tracking-widest mb-1">Open Ports</div>
        {ports.length === 0 ? (
          <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">No open ports detected (in scanned range)</div>
        ) : (
          <div className="flex flex-wrap gap-1">
            {ports
              .slice()
              .sort((a, b) => a.port - b.port)
              .map((p) => (
                <span
                  key={`${p.protocol}:${p.port}`}
                  className={`px-2 py-1 rounded-lg border text-[10px] font-mono ${
                    p.port >= 8281 && p.port <= 8284
                      ? 'bg-[rgb(var(--bg-muted-rgb)/0.4)] border-[rgb(var(--bg-steel-rgb)/0.4)] text-[var(--text-primary)]'
                      : 'bg-[rgb(var(--surface-rgb)/0.5)] border-[rgb(var(--bg-steel-rgb)/0.2)] text-[var(--text-secondary)]'
                  }`}
                  title={p.service ? `${p.protocol}/${p.port} (${p.service})` : `${p.protocol}/${p.port}`}
                >
                  {p.port}
                  {p.service ? `:${p.service}` : ''}
                </span>
              ))}
          </div>
        )}
      </div>
    </div>
  );
};

const NetworkMap: React.FC<NetworkMapProps> = ({ scan, loading, error }) => {
  return (
    <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4">
      <div className="flex items-center justify-between gap-2 mb-3">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[var(--bg-steel)]">radar</span>
          <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">Network Map</h3>
        </div>
        {loading && (
          <div className="text-[10px] text-[var(--text-secondary)] font-mono">Scanning…</div>
        )}
      </div>

      {error && (
        <div className="mb-3 text-[11px] text-[rgb(var(--danger-rgb)/0.85)] bg-[rgb(var(--surface-rgb)/0.7)] border border-[rgb(var(--danger-rgb)/0.3)] rounded-lg px-3 py-2">
          {error}
        </div>
      )}

      {!scan ? (
        <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">No scan results yet.</div>
      ) : (
        <>
          <div className="text-[11px] text-[var(--text-secondary)] mb-3">
            Target: <span className="font-mono font-bold">{scan.target}</span>
            <span className="opacity-70"> · </span>
            <span className="opacity-70">{new Date(scan.timestamp).toLocaleString()}</span>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
            {scan.hosts.length === 0 ? (
              <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">No hosts observed.</div>
            ) : (
              scan.hosts.map((h, idx) => <HostCard key={`${h.ipv4 || 'host'}-${idx}`} host={h} />)
            )}
          </div>
        </>
      )}
    </div>
  );
};

export default NetworkMap;


