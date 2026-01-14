import React, { useEffect, useRef, useState, useCallback } from 'react';
import ForceGraph3D from 'react-force-graph-3d';
import * as THREE from 'three';
import { useTheme } from '../context/ThemeContext';

interface PeerNode {
  node_id: string;
  software_version: string;
  manifest_hash: string;
  remote_address: string;
  status: 'Verified' | 'Pending' | 'Quarantined';
  last_seen: string;
}

interface QuarantineEntry {
  node_id: string;
  ip_address: string | null;
  reason: string;
  timestamp: string;
  quarantined_by: string;
}

interface ComplianceAlert {
  agent_id: string;
  manifest_hash: string;
  compliance_score: number;
  quarantined_by: string;
  timestamp: string;
}

interface TrustNetworkMapProps {
  className?: string;
}

const TrustNetworkMap: React.FC<TrustNetworkMapProps> = ({ className }) => {
  const { theme } = useTheme();
  const [nodes, setNodes] = useState<any[]>([]);
  const [links, setLinks] = useState<any[]>([]);
  const [selectedNode, setSelectedNode] = useState<PeerNode | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [meshHealth, setMeshHealth] = useState<any>(null);
  const [complianceAlerts, setComplianceAlerts] = useState<ComplianceAlert[]>([]);
  const [graphError, setGraphError] = useState<string | null>(null);
  const fgRef = useRef<any>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerSize, setContainerSize] = useState({ width: 800, height: 600 });
  const [isContainerReady, setIsContainerReady] = useState(false);
  const [shouldRenderGraph, setShouldRenderGraph] = useState(false);

  const [themeColors, setThemeColors] = useState(() => {
    const fallback = {
      success: 'rgb(var(--success-rgb))',
      warning: 'rgb(var(--warning-rgb))',
      danger: 'rgb(var(--danger-rgb))',
      accent: 'rgb(var(--accent-rgb))',
      textOnAccent: 'rgb(var(--text-on-accent-rgb))',
      overlaySolid: 'rgb(var(--overlay-rgb))',
      gridMajor: 'rgb(var(--bg-secondary-rgb))',
      gridMinor: 'rgb(var(--bg-muted-rgb))',
    };

    if (typeof window === 'undefined') return fallback;
    try {
      const s = window.getComputedStyle(document.documentElement);
      const pick = (name: string, fb: string) => (s.getPropertyValue(name).trim() || fb);
      const rgbTriplet = (name: string, fb: string) => {
        const raw = s.getPropertyValue(name).trim();
        const parts = raw.split(/\s+/).map((n) => Number(n)).filter((n) => Number.isFinite(n));
        if (parts.length >= 3) return `rgb(${parts[0]},${parts[1]},${parts[2]})`;
        return fb;
      };

      return {
        success: pick('--success', fallback.success),
        warning: pick('--warning', fallback.warning),
        danger: pick('--danger', fallback.danger),
        accent: pick('--accent', fallback.accent),
        textOnAccent: pick('--text-on-accent', fallback.textOnAccent),
        overlaySolid: rgbTriplet('--overlay-rgb', fallback.overlaySolid),
        gridMajor: rgbTriplet('--bg-secondary-rgb', fallback.gridMajor),
        gridMinor: rgbTriplet('--bg-muted-rgb', fallback.gridMinor),
      };
    } catch {
      return fallback;
    }
  });

  useEffect(() => {
    // Re-resolve CSS var colors when theme changes.
    setThemeColors((prev) => {
      try {
        const s = window.getComputedStyle(document.documentElement);
        const pick = (name: string, fb: string) => (s.getPropertyValue(name).trim() || fb);
        const rgbTriplet = (name: string, fb: string) => {
          const raw = s.getPropertyValue(name).trim();
          const parts = raw.split(/\s+/).map((n) => Number(n)).filter((n) => Number.isFinite(n));
          if (parts.length >= 3) return `rgb(${parts[0]},${parts[1]},${parts[2]})`;
          return fb;
        };

        return {
          ...prev,
          success: pick('--success', prev.success),
          warning: pick('--warning', prev.warning),
          danger: pick('--danger', prev.danger),
          accent: pick('--accent', prev.accent),
          textOnAccent: pick('--text-on-accent', prev.textOnAccent),
          overlaySolid: rgbTriplet('--overlay-rgb', prev.overlaySolid),
          gridMajor: rgbTriplet('--bg-secondary-rgb', prev.gridMajor),
          gridMinor: rgbTriplet('--bg-muted-rgb', prev.gridMinor),
        };
      } catch {
        return prev;
      }
    });
  }, [theme]);

  // Update container size when it changes
  useEffect(() => {
    const updateSize = () => {
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect();
        // Ensure we have valid dimensions (at least 100px)
        const width = Math.max(rect.width || 800, 100);
        const height = Math.max(rect.height || 600, 100);
        if (width > 0 && height > 0) {
          setContainerSize({ width, height });
          setIsContainerReady(true);
        }
      }
    };

    // Initial size check
    updateSize();
    
    // Use a delay to ensure DOM is fully ready before rendering the graph
    const timeoutId = setTimeout(() => {
      updateSize();
      setIsContainerReady(true);
      // Additional delay before rendering graph to ensure DOM is stable
      setTimeout(() => {
        if (containerRef.current) {
          const rect = containerRef.current.getBoundingClientRect();
          if (rect.width > 0 && rect.height > 0) {
            setShouldRenderGraph(true);
          }
        }
      }, 100);
    }, 200);
    
    const resizeObserver = new ResizeObserver(() => {
      updateSize();
      // Delay graph rendering after resize
      setTimeout(() => {
        if (containerRef.current) {
          const rect = containerRef.current.getBoundingClientRect();
          if (rect.width > 0 && rect.height > 0) {
            setShouldRenderGraph(true);
          }
        }
      }, 50);
    });
    
    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
    }

    // Also listen to window resize
    window.addEventListener('resize', updateSize);

    return () => {
      clearTimeout(timeoutId);
      resizeObserver.disconnect();
      window.removeEventListener('resize', updateSize);
    };
  }, []);

  // Fetch network topology and mesh health data
  const fetchNetworkData = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const [topologyResponse, meshHealthResponse, alertsResponse] = await Promise.all([
        fetch('/api/network/topology').catch(() => null),
        fetch('/api/network/mesh-health').catch(() => null),
        fetch('/api/network/compliance-alerts').catch(() => null),
      ]);

      // Handle topology data
      let topologyData = { nodes: [], links: [] };
      if (topologyResponse && topologyResponse.ok) {
        try {
          const text = await topologyResponse.text();
          if (text) {
            topologyData = JSON.parse(text);
          }
        } catch (err) {
          console.warn('Failed to parse topology data:', err);
        }
      }

      // Handle mesh health data
      let meshHealthData = null;
      if (meshHealthResponse && meshHealthResponse.ok) {
        try {
          const text = await meshHealthResponse.text();
          if (text) {
            meshHealthData = JSON.parse(text);
            setMeshHealth(meshHealthData);
          }
        } catch (err) {
          console.warn('Failed to parse mesh health data:', err);
        }
      }

      // Handle compliance alerts
      if (alertsResponse && alertsResponse.ok) {
        try {
          const text = await alertsResponse.text();
          if (text) {
            const alertsData = JSON.parse(text);
            setComplianceAlerts(Array.isArray(alertsData) ? alertsData : []);
          } else {
            setComplianceAlerts([]);
          }
        } catch (err) {
          console.warn('Failed to parse compliance alerts:', err);
          setComplianceAlerts([]);
        }
      } else {
        setComplianceAlerts([]);
      }

      // Transform nodes for 3D graph
      const nodesArray = (topologyData.nodes || []).map((node: any) => ({
        id: node.id || node.node_id,
        label: (node.node_id || node.id || '').substring(0, 8),
        status: node.status,
        software_version: node.software_version,
        manifest_hash: node.manifest_hash,
        remote_address: node.remote_address,
        last_seen: node.last_seen,
        // Position will be set by force simulation
        x: Math.random() * 1000 - 500,
        y: Math.random() * 1000 - 500,
        z: Math.random() * 1000 - 500,
      }));

      // Transform links for 3D graph
      const linksArray = (topologyData.links || []).map((link: any) => ({
        source: link.source,
        target: link.target,
        id: `${link.source}-${link.target}`,
        type: link.type || 'trust',
      }));

      setNodes(nodesArray);
      setLinks(linksArray);
    } catch (err) {
      console.error('Failed to fetch network data:', err);
      setError('Failed to load network data');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchNetworkData();
    // Refresh every 10 seconds
    const interval = setInterval(fetchNetworkData, 10000);
    return () => clearInterval(interval);
  }, [fetchNetworkData]);

  // Catch errors from ForceGraph3D
  useEffect(() => {
    const handleError = (event: ErrorEvent) => {
      if (event.message && event.message.includes('react-force-graph-3d')) {
        console.error('ForceGraph3D error caught:', event.error);
        setGraphError('Failed to initialize 3D graph. Please refresh the page.');
      }
    };

    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      if (event.reason && event.reason.toString().includes('style')) {
        console.error('ForceGraph3D promise rejection:', event.reason);
        setGraphError('Graph rendering error. The container may not be ready.');
      }
    };

    window.addEventListener('error', handleError);
    window.addEventListener('unhandledrejection', handleUnhandledRejection);

    return () => {
      window.removeEventListener('error', handleError);
      window.removeEventListener('unhandledrejection', handleUnhandledRejection);
    };
  }, []);

  // Node color based on status
  const getNodeColor = (node: any): string => {
    switch (node.status) {
      case 'Verified':
        return themeColors.success;
      case 'Pending':
        return themeColors.warning;
      case 'Quarantined':
        return themeColors.danger;
      default:
        return 'rgb(var(--text-secondary-rgb) / 0.8)';
    }
  };

  // Link color based on type
  const getLinkColor = (link: any): string => {
    if (link.type === 'weak') {
      return themeColors.warning;
    }
    return themeColors.success;
  };

  // Custom node 3D object with glow effect for verified nodes
  const nodeThreeObject = (node: any) => {
    const color = getNodeColor(node);
    const isVerified = node.status === 'Verified';
    const isQuarantined = node.status === 'Quarantined';

    // Create a sphere geometry
    const geometry = new THREE.SphereGeometry(8, 16, 16);
    const material = new THREE.MeshPhongMaterial({
      color,
      emissive: isVerified ? themeColors.accent : themeColors.overlaySolid,
      emissiveIntensity: isVerified ? 0.5 : 0,
      shininess: isVerified ? 100 : 30,
    });

    const mesh = new THREE.Mesh(geometry, material);

    // Add glow effect for verified nodes (blue pulse)
    if (isVerified) {
      const glowGeometry = new THREE.SphereGeometry(10, 16, 16);
      const glowMaterial = new THREE.MeshBasicMaterial({
        color: themeColors.accent,
        transparent: true,
        opacity: 0.3,
      });
      const glow = new THREE.Mesh(glowGeometry, glowMaterial);
      mesh.add(glow);
    }

    // Add skull icon overlay for quarantined nodes (simplified as a red cross)
    if (isQuarantined) {
      // Create a simple cross indicator
      const crossGeometry = new THREE.BoxGeometry(2, 12, 0.5);
      const crossMaterial = new THREE.MeshBasicMaterial({ color: themeColors.textOnAccent });
      const cross1 = new THREE.Mesh(crossGeometry, crossMaterial);
      cross1.rotation.z = Math.PI / 4;
      cross1.position.y = 10;
      mesh.add(cross1);
      
      const cross2 = new THREE.Mesh(crossGeometry, crossMaterial);
      cross2.rotation.z = -Math.PI / 4;
      cross2.position.y = 10;
      mesh.add(cross2);
    }

    return mesh;
  };

  return (
    <div 
      ref={containerRef} 
      className={`relative ${className || ''}`} 
      style={{ 
        height: '100%', 
        width: '100%', 
        minHeight: '400px',
        position: 'relative',
        overflow: 'hidden'
      }}
    >
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-[rgb(var(--overlay-rgb)/0.5)] z-10">
          <div className="text-[var(--text-on-accent)] text-sm">Loading network map...</div>
        </div>
      )}

      {error && (
        <div className="absolute top-4 left-4 bg-[rgb(var(--danger-rgb)/0.8)] text-[var(--text-on-accent)] px-4 py-2 rounded z-10">
          {error}
        </div>
      )}

      {nodes.length === 0 && !loading && !error && (
        <div className="absolute inset-0 flex items-center justify-center bg-[rgb(var(--overlay-rgb)/0.3)] z-10">
          <div className="text-[rgb(var(--text-on-accent-rgb)/0.7)] text-sm">No peers discovered yet</div>
        </div>
      )}

      {graphError && (
        <div className="absolute inset-0 flex items-center justify-center bg-[rgb(var(--overlay-rgb)/0.5)] z-10">
          <div className="bg-[rgb(var(--danger-rgb)/0.8)] text-[var(--text-on-accent)] px-4 py-2 rounded">
            <div className="text-sm font-bold mb-1">Graph Rendering Error</div>
            <div className="text-xs">{graphError}</div>
          </div>
        </div>
      )}

      {(containerSize.width > 0 && containerSize.height > 0 && isContainerReady && shouldRenderGraph && !graphError) && (
        <ForceGraph3D
          ref={fgRef}
          graphData={{ nodes, links }}
          nodeLabel={(node: any) => `${node.label}\n${node.status}`}
          nodeColor={getNodeColor}
          nodeThreeObject={nodeThreeObject}
          linkColor={getLinkColor}
          linkWidth={(link: any) => link.type === 'weak' ? 1 : 2}
          linkOpacity={(link: any) => link.type === 'weak' ? 0.3 : 0.6}
          linkCurvature={(link: any) => link.type === 'weak' ? 0.3 : 0}
          onNodeClick={(node: any) => {
            setSelectedNode(node as any);
          }}
          backgroundColor={themeColors.overlaySolid}
          showNavInfo={false}
          width={containerSize.width}
          height={containerSize.height}
          // Cyber-mesh grid floor
          cooldownTicks={100}
          onEngineStop={() => {
            if (fgRef.current) {
              fgRef.current.zoomToFit(400);
            }
          }}
          // Add grid helper
          extraRenderers={[
            (scene: THREE.Scene) => {
              try {
                const gridHelper = new THREE.GridHelper(2000, 50, themeColors.gridMajor, themeColors.gridMinor);
                gridHelper.position.y = -500;
                scene.add(gridHelper);
                return () => {
                  try {
                    scene.remove(gridHelper);
                  } catch (e) {
                    // Ignore cleanup errors
                  }
                };
              } catch (e) {
                console.warn('Failed to create grid helper:', e);
                return () => {};
              }
            },
          ]}
        />
      )}

      {(!isContainerReady || (containerSize.width === 0 || containerSize.height === 0)) && !loading && !graphError && (
        <div className="absolute inset-0 flex items-center justify-center bg-[rgb(var(--overlay-rgb)/0.3)] z-10">
          <div className="text-[rgb(var(--text-on-accent-rgb)/0.7)] text-sm">Initializing network visualization...</div>
        </div>
      )}

      {/* Side panel for node details */}
      {selectedNode && (
        <div className="absolute top-4 right-4 bg-[rgb(var(--text-primary-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 w-80 z-20 text-[var(--text-on-accent)]">
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-sm font-bold uppercase tracking-widest text-[var(--bg-steel)]">
              Node Details
            </h3>
            <button
              onClick={() => setSelectedNode(null)}
              className="text-[rgb(var(--text-on-accent-rgb)/0.7)] hover:text-[var(--text-on-accent)]"
            >
              ×
            </button>
          </div>

          <div className="space-y-2 text-xs">
            <div>
              <div className="text-[var(--bg-steel)] font-bold uppercase text-[10px] tracking-wider mb-1">
                Node ID
              </div>
              <div className="font-mono text-[11px] break-all">{selectedNode.node_id}</div>
            </div>

            <div>
              <div className="text-[var(--bg-steel)] font-bold uppercase text-[10px] tracking-wider mb-1">
                Status
              </div>
              <div
                className={`inline-block px-2 py-1 rounded text-[10px] font-bold ${
                  selectedNode.status === 'Verified'
                    ? 'bg-[rgb(var(--success-rgb)/0.2)] text-[rgb(var(--success-rgb)/0.9)]'
                    : selectedNode.status === 'Pending'
                    ? 'bg-[rgb(var(--warning-rgb)/0.2)] text-[rgb(var(--warning-rgb)/0.95)]'
                    : 'bg-[rgb(var(--danger-rgb)/0.2)] text-[rgb(var(--danger-rgb)/0.9)]'
                }`}
              >
                {selectedNode.status}
              </div>
            </div>

            <div>
              <div className="text-[var(--bg-steel)] font-bold uppercase text-[10px] tracking-wider mb-1">
                Software Version
              </div>
              <div className="font-mono text-[11px]">{selectedNode.software_version || 'N/A'}</div>
            </div>

            <div>
              <div className="text-[var(--bg-steel)] font-bold uppercase text-[10px] tracking-wider mb-1">
                Manifest Hash
              </div>
              <div className="font-mono text-[11px] break-all">
                {selectedNode.manifest_hash || 'N/A'}
              </div>
            </div>

            <div>
              <div className="text-[var(--bg-steel)] font-bold uppercase text-[10px] tracking-wider mb-1">
                Remote Address
              </div>
              <div className="font-mono text-[11px]">{selectedNode.remote_address}</div>
            </div>

            <div>
              <div className="text-[var(--bg-steel)] font-bold uppercase text-[10px] tracking-wider mb-1">
                Last Seen
              </div>
              <div className="text-[11px]">
                {new Date(selectedNode.last_seen).toLocaleString()}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Mesh Health Summary Card */}
      {meshHealth && (
        <div className="absolute top-4 left-4 bg-[rgb(var(--text-primary-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4 w-80 z-20">
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-sm font-bold uppercase tracking-widest text-[var(--bg-steel)]">
              Blue Flame Mesh Health
            </h3>
          </div>
          <div className="space-y-2 text-xs">
            <div className="flex justify-between">
              <span className="text-[rgb(var(--text-on-accent-rgb)/0.7)]">Total Nodes:</span>
              <span className="text-[var(--text-on-accent)] font-mono">{meshHealth.total_nodes || 0}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-[rgb(var(--text-on-accent-rgb)/0.7)]">Aligned Nodes:</span>
              <span className="text-[rgb(var(--success-rgb)/0.85)] font-mono">{meshHealth.aligned_nodes || 0}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-[rgb(var(--text-on-accent-rgb)/0.7)]">Quarantined:</span>
              <span className="text-[rgb(var(--danger-rgb)/0.8)] font-mono">{meshHealth.quarantined_nodes || 0}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-[rgb(var(--text-on-accent-rgb)/0.7)]">Alignment Drift:</span>
              <span className={`font-mono ${(meshHealth.alignment_drift_percentage || 0) > 10 ? 'text-[rgb(var(--warning-rgb)/0.85)]' : 'text-[rgb(var(--success-rgb)/0.85)]'}`}>
                {(meshHealth.alignment_drift_percentage || 0).toFixed(1)}%
              </span>
            </div>
            <div className="pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.3)] text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.5)]">
              Updated: {new Date(meshHealth.last_updated_utc || Date.now()).toLocaleTimeString()}
            </div>
          </div>
        </div>
      )}

      {/* Network-Wide Compliance Alerts */}
      {complianceAlerts.length > 0 && (
        <div className="absolute top-4 right-4 bg-[rgb(var(--danger-rgb)/0.9)] border border-[rgb(var(--danger-rgb)/0.5)] rounded-lg p-3 z-20 max-w-md">
          <div className="text-xs font-bold uppercase tracking-widest text-[var(--text-on-accent)] mb-2 flex items-center gap-2">
            <span className="material-symbols-outlined text-sm">warning</span>
            Network-Wide Alert
          </div>
          <div className="space-y-2 max-h-64 overflow-y-auto">
            {complianceAlerts.slice(0, 5).map((alert, idx) => (
              <div key={idx} className="bg-[rgb(var(--overlay-rgb)/0.3)] rounded p-2 border border-[rgb(var(--danger-rgb)/0.3)]">
                <div className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.9)] font-mono mb-1">
                  Agent: {alert.agent_id.substring(0, 12)}...
                </div>
                <div className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.7)] mb-1">
                  Manifest: {alert.manifest_hash.substring(0, 16)}...
                </div>
                <div className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.7)] mb-1">
                  Score: <span className="text-[rgb(var(--text-on-accent-rgb)/0.95)] font-bold">{alert.compliance_score.toFixed(1)}%</span>
                </div>
                <div className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.5)]">
                  Rejected by: {alert.quarantined_by.substring(0, 8)}...
                </div>
                <div className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.5)]">
                  {new Date(alert.timestamp).toLocaleString()}
                </div>
              </div>
            ))}
            {complianceAlerts.length > 5 && (
              <div className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.5)] text-center">
                +{complianceAlerts.length - 5} more alerts
              </div>
            )}
          </div>
        </div>
      )}

      {/* Legend */}
      <div className="absolute bottom-4 left-4 bg-[rgb(var(--text-primary-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-3 z-20">
        <div className="text-xs font-bold uppercase tracking-widest text-[var(--bg-steel)] mb-2">
          Legend
        </div>
        <div className="space-y-1 text-[10px]">
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-[var(--success)]"></div>
            <span className="text-[rgb(var(--text-on-accent-rgb)/0.8)]">Verified</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-[var(--warning)]"></div>
            <span className="text-[rgb(var(--text-on-accent-rgb)/0.8)]">Pending</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-[var(--danger)]"></div>
            <span className="text-[rgb(var(--text-on-accent-rgb)/0.8)]">Quarantined</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default TrustNetworkMap;
