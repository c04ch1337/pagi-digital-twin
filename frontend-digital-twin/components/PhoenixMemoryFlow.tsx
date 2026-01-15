import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Brain, Network, Activity, Shield } from 'lucide-react';
import { useTheme } from '../context/ThemeContext';
import { formatCompactBytes, formatCompactNumber } from '../utils/formatMetrics';

interface MemoryTransfer {
  source_node: string;
  destination_node: string;
  topic: string;
  fragments_count: number;
  bytes_transferred: number;
  redacted_entities_count: number;
  timestamp: string;
}

interface Node {
  id: string;
  x: number;
  y: number;
  label: string;
  status?: 'online' | 'offline' | 'quarantined';
}

interface Pulse {
  id: string;
  from: string;
  to: string;
  progress: number; // 0 to 1
  topic: string;
  timestamp: number;
  redactedCount?: number;
  x?: number; // Current position for tooltip
  y?: number;
}

interface MemoryStats {
  bytes_transferred_24h: number;
  fragments_exchanged_24h: number;
  active_transfers: number;
  total_nodes: number;
}

interface KnowledgeSnippet {
  topic: string;
  timestamp: string;
  redacted_count: number;
}

const PhoenixMemoryFlow: React.FC = () => {
  const { theme } = useTheme();
  const [nodes, setNodes] = useState<Node[]>([]);
  const [pulses, setPulses] = useState<Pulse[]>([]);
  const [knowledgeSnippets, setKnowledgeSnippets] = useState<KnowledgeSnippet[]>([]);
  const [stats, setStats] = useState<MemoryStats | null>(null);
  const [redactedCount, setRedactedCount] = useState(0);
  const [isConnected, setIsConnected] = useState(false);
  const [meshHealth, setMeshHealth] = useState<any>(null);
  const [nodeVolumes, setNodeVolumes] = useState<Record<string, number>>({});
  const [hoveredPulse, setHoveredPulse] = useState<Pulse | null>(null);
  const [mousePos, setMousePos] = useState({ x: 0, y: 0 });
  const [pruningTopic, setPruningTopic] = useState<string | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animationFrameRef = useRef<number | undefined>(undefined);
  const eventSourceRef = useRef<EventSource | null>(null);

  const [canvasPalette, setCanvasPalette] = useState(() => {
    const fallback = {
      success: [16, 185, 129],
      danger: [239, 68, 68],
      textSecondary: [22, 50, 71],
      textOnAccent: [255, 255, 255],
    };

    if (typeof window === 'undefined') return fallback;
    try {
      const s = window.getComputedStyle(document.documentElement);
      const rgbTriplet = (name: string, fb: number[]) => {
        const raw = s.getPropertyValue(name).trim();
        const parts = raw.split(/\s+/).map((n) => Number(n)).filter((n) => Number.isFinite(n));
        if (parts.length >= 3) return [parts[0], parts[1], parts[2]] as number[];
        return fb;
      };
      return {
        success: rgbTriplet('--success-rgb', fallback.success),
        danger: rgbTriplet('--danger-rgb', fallback.danger),
        textSecondary: rgbTriplet('--text-secondary-rgb', fallback.textSecondary),
        textOnAccent: rgbTriplet('--text-on-accent-rgb', fallback.textOnAccent),
      };
    } catch {
      return fallback;
    }
  });

  useEffect(() => {
    // Update canvas palette when theme changes.
    if (typeof window === 'undefined') return;
    try {
      const s = window.getComputedStyle(document.documentElement);
      const rgbTriplet = (name: string, fb: number[]) => {
        const raw = s.getPropertyValue(name).trim();
        const parts = raw.split(/\s+/).map((n) => Number(n)).filter((n) => Number.isFinite(n));
        if (parts.length >= 3) return [parts[0], parts[1], parts[2]] as number[];
        return fb;
      };
      setCanvasPalette((prev) => ({
        ...prev,
        success: rgbTriplet('--success-rgb', prev.success),
        danger: rgbTriplet('--danger-rgb', prev.danger),
        textSecondary: rgbTriplet('--text-secondary-rgb', prev.textSecondary),
        textOnAccent: rgbTriplet('--text-on-accent-rgb', prev.textOnAccent),
      }));
    } catch {
      // ignore
    }
  }, [theme]);

  // Fetch initial node list and stats
  const fetchInitialData = useCallback(async () => {
    try {
      // Fetch network topology and mesh health in parallel
      const [topologyResponse, meshHealthResponse] = await Promise.all([
        fetch('/api/network/topology').catch(() => null),
        fetch('/api/network/mesh-health').catch(() => null),
      ]);

      // Parse mesh health data
      let healthData: any = null;
      if (meshHealthResponse) {
        try {
          healthData = await meshHealthResponse.json();
          setMeshHealth(healthData);
        } catch (e) {
          console.error('[PhoenixMemoryFlow] Failed to parse mesh health:', e);
        }
      }

      // Get node statuses from mesh health (if available)
      const nodeStatusMap = new Map<string, 'online' | 'offline' | 'quarantined'>();
      if (healthData?.peers) {
        healthData.peers.forEach((peer: any) => {
          const nodeId = peer.node_id || peer.id;
          if (nodeId) {
            // Determine status based on peer data
            if (peer.quarantined) {
              nodeStatusMap.set(nodeId, 'quarantined');
            } else if (peer.status === 'verified' || peer.status === 'online') {
              nodeStatusMap.set(nodeId, 'online');
            } else {
              nodeStatusMap.set(nodeId, 'offline');
            }
          }
        });
      }

      // Fetch network topology to get nodes
      if (topologyResponse) {
        const topologyData = await topologyResponse.json().catch(() => ({ nodes: [], links: [] }));
        const nodeList = (topologyData.nodes || []).map((node: any, index: number) => {
          const nodeId = node.node_id || node.id || `node-${index}`;
          return {
            id: nodeId,
            x: 0, // Will be calculated
            y: 0, // Will be calculated
            label: nodeId.substring(0, 8),
            status: nodeStatusMap.get(nodeId) || (node.status === 'offline' ? 'offline' : 'online') as 'online' | 'offline' | 'quarantined',
          };
        });
        
        // Arrange nodes in a circle
        const centerX = 400;
        const centerY = 300;
        const radius = 200;
        const arrangedNodes = nodeList.map((node: Node, index: number) => {
          const angle = (index / nodeList.length) * 2 * Math.PI;
          return {
            ...node,
            x: centerX + radius * Math.cos(angle),
            y: centerY + radius * Math.sin(angle),
          };
        });
        setNodes(arrangedNodes);
      }

      // Fetch memory stats
      const statsResponse = await fetch('/api/phoenix/memory/stats').catch(() => null);
      if (statsResponse) {
        const statsData = await statsResponse.json().catch(() => null);
        if (statsData) {
          setStats(statsData);
        }
      }

      // Fetch heat map data
      const heatMapResponse = await fetch('/api/phoenix/memory/heatmap').catch(() => null);
      if (heatMapResponse) {
        const heatMapData = await heatMapResponse.json().catch(() => null);
        if (heatMapData?.node_volumes) {
          setNodeVolumes(heatMapData.node_volumes);
        }
      }
    } catch (error) {
      console.error('[PhoenixMemoryFlow] Failed to fetch initial data:', error);
    }
  }, []);

  // Periodically refresh node health status
  useEffect(() => {
    const interval = setInterval(() => {
      fetchInitialData();
    }, 10000); // Refresh every 10 seconds

    return () => clearInterval(interval);
  }, [fetchInitialData]);

  // Connect to SSE stream
  useEffect(() => {
    const sseUrl = '/api/phoenix/stream';
    console.log('[PhoenixMemoryFlow] Connecting to SSE stream:', sseUrl);

    const eventSource = new EventSource(sseUrl);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      console.log('[PhoenixMemoryFlow] SSE connection established');
      setIsConnected(true);
    };

    eventSource.addEventListener('memory_transfer', (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data);
        if (data.source_node && data.destination_node) {
          // Create a new pulse animation
          const pulse: Pulse = {
            id: `pulse-${Date.now()}-${Math.random()}`,
            from: data.source_node,
            to: data.destination_node,
            progress: 0,
            topic: data.topic || 'Unknown',
            timestamp: Date.now(),
            redactedCount: data.redacted_entities_count || 0,
          };
          setPulses(prev => [...prev, pulse]);

          // Add knowledge snippet
          setKnowledgeSnippets(prev => [
            {
              topic: data.topic || 'Unknown',
              timestamp: data.timestamp || new Date().toISOString(),
              redacted_count: data.redacted_entities_count || 0,
            },
            ...prev.slice(0, 9), // Keep last 10
          ]);

          // Update redacted count
          setRedactedCount(prev => prev + (data.redacted_entities_count || 0));

          // Refresh heat map data
          fetch('/api/phoenix/memory/heatmap')
            .then(res => res.json())
            .then(heatMapData => {
              if (heatMapData?.node_volumes) {
                setNodeVolumes(heatMapData.node_volumes);
              }
            })
            .catch(() => {});
        }
      } catch (error) {
        console.error('[PhoenixMemoryFlow] Failed to parse memory transfer event:', error);
      }
    });

    eventSource.addEventListener('consensus_vote', (event: MessageEvent) => {
      // Handle consensus votes if needed
      console.log('[PhoenixMemoryFlow] Consensus vote received');
    });

    eventSource.addEventListener('quarantine_alert', (event: MessageEvent) => {
      // Handle quarantine alerts if needed
      console.log('[PhoenixMemoryFlow] Quarantine alert received');
    });

    eventSource.onerror = (error) => {
      console.error('[PhoenixMemoryFlow] SSE error:', error);
      setIsConnected(false);
    };

    fetchInitialData();

    return () => {
      console.log('[PhoenixMemoryFlow] Cleaning up SSE connection');
      eventSource.close();
      eventSourceRef.current = null;
    };
  }, [fetchInitialData]);

  // Animation loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const rgba = (rgb: number[], a: number) => `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${a})`;

    const animate = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // Draw connections between nodes
      ctx.strokeStyle = rgba(canvasPalette.success, 0.2);
      ctx.lineWidth = 1;
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          ctx.beginPath();
          ctx.moveTo(nodes[i].x, nodes[i].y);
          ctx.lineTo(nodes[j].x, nodes[j].y);
          ctx.stroke();
        }
      }

      // Draw nodes
      nodes.forEach(node => {
        // Determine node color based on status
        let nodeColor = rgba(canvasPalette.success, 0.8);
        let strokeColor = rgba(canvasPalette.success, 1);
        
        if (node.status === 'offline') {
          nodeColor = rgba(canvasPalette.textSecondary, 0.35);
          strokeColor = rgba(canvasPalette.textSecondary, 0.6);
        } else if (node.status === 'quarantined') {
          nodeColor = rgba(canvasPalette.danger, 0.6);
          strokeColor = rgba(canvasPalette.danger, 1);
        }

        // Calculate glow intensity based on node volume (heat map)
        const nodeVolume = nodeVolumes[node.id] || 0;
        const maxVolume = Math.max(
          ...Object.values(nodeVolumes).map((v) => (typeof v === 'number' && Number.isFinite(v) ? v : 0)),
          1
        );
        const glowIntensity = nodeVolume / maxVolume; // 0 to 1
        const glowRadius = 20 + (glowIntensity * 15); // 20 to 35 pixels

        // Draw glow effect for high-volume nodes
        if (nodeVolume > 0 && node.status !== 'offline' && node.status !== 'quarantined') {
          const glowGradient = ctx.createRadialGradient(node.x, node.y, 20, node.x, node.y, glowRadius);
          glowGradient.addColorStop(0, rgba(canvasPalette.success, 0.3 * glowIntensity));
          glowGradient.addColorStop(0.5, rgba(canvasPalette.success, 0.15 * glowIntensity));
          glowGradient.addColorStop(1, rgba(canvasPalette.success, 0));
          
          ctx.fillStyle = glowGradient;
          ctx.beginPath();
          ctx.arc(node.x, node.y, glowRadius, 0, 2 * Math.PI);
          ctx.fill();
        }

        // Node circle
        ctx.beginPath();
        ctx.arc(node.x, node.y, 20, 0, 2 * Math.PI);
        ctx.fillStyle = nodeColor;
        ctx.fill();
        ctx.strokeStyle = strokeColor;
        ctx.lineWidth = 2;
        ctx.stroke();

        // Node label
        ctx.fillStyle = node.status === 'offline'
          ? rgba(canvasPalette.textSecondary, 0.8)
          : rgba(canvasPalette.textOnAccent, 0.9);
        ctx.font = '10px monospace';
        ctx.textAlign = 'center';
        ctx.fillText(node.label, node.x, node.y + 35);
      });

      // Draw and update pulses (only for online nodes)
      setPulses(prevPulses => {
        const updatedPulses: Pulse[] = [];
        prevPulses.forEach(pulse => {
          const fromNode = nodes.find(n => n.id === pulse.from);
          const toNode = nodes.find(n => n.id === pulse.to);
          
          // Only animate pulses between online nodes
          if (fromNode && toNode && 
              fromNode.status !== 'offline' && toNode.status !== 'offline') {
            const newProgress = pulse.progress + 0.02; // Animation speed
            
            if (newProgress < 1) {
              // Draw pulse
              const x = fromNode.x + (toNode.x - fromNode.x) * newProgress;
              const y = fromNode.y + (toNode.y - fromNode.y) * newProgress;
              
              // Store position for tooltip
              pulse.x = x;
              pulse.y = y;
              
              // Pulse glow effect
              const gradient = ctx.createRadialGradient(x, y, 0, x, y, 15);
              gradient.addColorStop(0, rgba(canvasPalette.success, 0.8));
              gradient.addColorStop(0.5, rgba(canvasPalette.success, 0.4));
              gradient.addColorStop(1, rgba(canvasPalette.success, 0));
              
              ctx.fillStyle = gradient;
              ctx.beginPath();
              ctx.arc(x, y, 15, 0, 2 * Math.PI);
              ctx.fill();
              
              updatedPulses.push({ ...pulse, progress: newProgress, x, y });
            }
            // Remove completed pulses
          }
          // Remove pulses involving offline nodes
        });
        return updatedPulses;
      });

      animationFrameRef.current = requestAnimationFrame(animate);
    };

    animate();

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [nodes, nodeVolumes, pulses, canvasPalette]);

  // Handle mouse move for pulse tooltips
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const handleMouseMove = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      setMousePos({ x, y });

      // Check if mouse is over a pulse
      const hovered = pulses.find(pulse => {
        if (pulse.x === undefined || pulse.y === undefined) return false;
        const dx = x - pulse.x;
        const dy = y - pulse.y;
        return Math.sqrt(dx * dx + dy * dy) < 15; // Within pulse radius
      });

      setHoveredPulse(hovered || null);
    };

    canvas.addEventListener('mousemove', handleMouseMove);
    return () => canvas.removeEventListener('mousemove', handleMouseMove);
  }, [pulses]);


  const handlePruneTopic = async (topic: string) => {
    if (!confirm(`Are you sure you want to prune topic "${topic}"? This will delete all vectors related to this topic from Qdrant collections.`)) {
      return;
    }

    setPruningTopic(topic);
    try {
      const response = await fetch('/api/phoenix/memory/prune', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ topic }),
      });

      const data = await response.json();
      if (data.success) {
        // Refresh heat map data
        fetch('/api/phoenix/memory/heatmap')
          .then(res => res.json())
          .then(heatMapData => {
            if (heatMapData?.node_volumes) {
              setNodeVolumes(heatMapData.node_volumes);
            }
          })
          .catch(() => {});
        
        // Clear hovered pulse
        setHoveredPulse(null);
        
        alert(`Topic "${topic}" pruned successfully. ${data.deleted_count} vectors deleted.`);
      } else {
        alert(`Failed to prune topic: ${data.message}`);
      }
    } catch (error) {
      console.error('Failed to prune topic:', error);
      alert('Failed to prune topic. Please try again.');
    } finally {
      setPruningTopic(null);
    }
  };

  return (
    <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-4">
      <div className="flex items-center justify-between gap-2 mb-4">
        <div className="flex items-center gap-2">
          <Brain className="w-4 h-4 text-[var(--bg-steel)]" />
          <h3 className="text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)]">
            Phoenix Memory Flow
          </h3>
        </div>
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-[var(--success)]' : 'bg-[var(--danger)]'}`} />
          <span className="text-[10px] text-[var(--text-secondary)]">
            {isConnected ? 'LIVE' : 'OFFLINE'}
          </span>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Network Graph */}
        <div className="lg:col-span-2 bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg p-4 relative overflow-hidden">
          <canvas
            ref={canvasRef}
            width={800}
            height={600}
            className="w-full h-full"
            style={{ maxHeight: '400px' }}
          />
          {nodes.length === 0 && (
            <div className="absolute inset-0 flex items-center justify-center text-[var(--text-secondary)] text-sm">
              No nodes detected. Waiting for network topology...
            </div>
          )}
          {/* Pulse Tooltip */}
          {hoveredPulse && hoveredPulse.x !== undefined && hoveredPulse.y !== undefined && (
            <div
              className="absolute bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.5)] rounded-lg p-3 shadow-lg z-10"
              style={{
                left: `${hoveredPulse.x + 20}px`,
                top: `${hoveredPulse.y - 10}px`,
                transform: 'translateY(-100%)',
              }}
            >
              <div className="text-xs font-bold text-[var(--text-primary)] mb-1">
                Topic: {hoveredPulse.topic}
              </div>
              {hoveredPulse.redactedCount !== undefined && hoveredPulse.redactedCount > 0 && (
                <div className="text-[10px] text-[var(--bg-steel)] mb-2">
                  Redactions: {formatCompactNumber(hoveredPulse.redactedCount)}
                </div>
              )}
              <button
                onClick={() => handlePruneTopic(hoveredPulse.topic)}
                disabled={pruningTopic === hoveredPulse.topic}
                className="w-full px-2 py-1 text-[10px] bg-[var(--danger)] hover:bg-[rgb(var(--danger-rgb)/0.85)] disabled:bg-[rgb(var(--surface-rgb)/0.35)] disabled:cursor-not-allowed text-[var(--text-on-accent)] rounded transition-colors"
              >
                {pruningTopic === hoveredPulse.topic ? 'Pruning...' : 'Prune Topic'}
              </button>
            </div>
          )}
        </div>

        {/* Side Panel */}
        <div className="space-y-4">
          {/* Stats Card */}
          <div className="bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg p-3">
            <div className="flex items-center gap-2 mb-2">
              <Activity className="w-3 h-3 text-[var(--bg-steel)]" />
              <h4 className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                Memory Stats (24h)
              </h4>
            </div>
            {stats ? (
              <div className="space-y-1 text-[11px]">
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Transferred:</span>
                  <span className="font-mono font-bold text-[var(--text-primary)]">
                    {formatCompactBytes(stats.bytes_transferred_24h)}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Fragments:</span>
                  <span className="font-mono font-bold text-[var(--text-primary)]">
                    {formatCompactNumber(stats.fragments_exchanged_24h)}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Active:</span>
                  <span className="font-mono font-bold text-[var(--text-primary)]">
                    {stats.active_transfers}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-secondary)]">Nodes:</span>
                  <span className="font-mono font-bold text-[var(--text-primary)]">
                    {stats.total_nodes}
                  </span>
                </div>
              </div>
            ) : (
              <div className="text-[11px] text-[var(--text-secondary)] opacity-70">
                Loading stats...
              </div>
            )}
          </div>

          {/* Scrubbing Log */}
          <div className="bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg p-3">
            <div className="flex items-center gap-2 mb-2">
              <Shield className="w-3 h-3 text-[var(--bg-steel)]" />
              <h4 className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                Scrubbing Log
              </h4>
            </div>
            <div className="text-[11px]">
              <div className="font-mono font-bold text-[var(--text-primary)] text-lg">
                {formatCompactNumber(redactedCount)}
              </div>
              <div className="text-[var(--text-secondary)] opacity-80">
                Sensitive entities redacted
              </div>
            </div>
          </div>

          {/* Knowledge Snippets */}
          <div className="bg-[rgb(var(--surface-rgb)/0.4)] rounded-lg p-3">
            <div className="flex items-center gap-2 mb-2">
              <Network className="w-3 h-3 text-[var(--bg-steel)]" />
              <h4 className="text-[10px] font-bold uppercase tracking-widest text-[var(--text-secondary)]">
                Knowledge Snippets
              </h4>
            </div>
            <div className="space-y-2 max-h-48 overflow-y-auto">
              {knowledgeSnippets.length === 0 ? (
                <div className="text-[11px] text-[var(--text-secondary)] opacity-70 italic">
                  No knowledge exchanges yet...
                </div>
              ) : (
                knowledgeSnippets.map((snippet, index) => (
                  <div
                    key={index}
                    className="bg-[rgb(var(--surface-rgb)/0.6)] rounded p-2 border border-[rgb(var(--bg-steel-rgb)/0.2)]"
                  >
                    <div className="text-[11px] font-bold text-[var(--text-primary)] mb-1">
                      {snippet.topic}
                    </div>
                    <div className="text-[9px] text-[var(--text-secondary)] opacity-70">
                      {new Date(snippet.timestamp).toLocaleTimeString()}
                    </div>
                    {snippet.redacted_count > 0 && (
                      <div className="text-[9px] text-[var(--bg-steel)] mt-1">
                        {formatCompactNumber(snippet.redacted_count)} entities redacted
                      </div>
                    )}
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default PhoenixMemoryFlow;
