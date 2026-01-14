import React, { useEffect, useRef, useState, useCallback } from 'react';
import ForceGraph3D from 'react-force-graph-3d';
import * as THREE from 'three';
import { useTheme } from '../context/ThemeContext';
import { SearchResult } from '../services/phoenixSearchService';
import { Brain, X, ZoomIn, ZoomOut, RotateCcw, Info, Route, Sparkles } from 'lucide-react';

interface KnowledgeNode {
  id: string;
  x?: number;
  y?: number;
  z?: number;
  vx?: number;
  vy?: number;
  vz?: number;
  fx?: number;
  fy?: number;
  fz?: number;
  // Search result data
  title: string;
  type: 'chat' | 'memory' | 'playbook';
  content: string;
  snippet?: string;
  // Visualization properties
  confidence: number; // Cross-Encoder score (0-1)
  similarity: number; // Dense embedding similarity (0-1)
  size: number;
  color: string;
}

interface KnowledgeEdge {
  source: string;
  target: string;
  strength: number;
}

interface PathStep {
  node_id: string;
  title: string;
  snippet?: string;
  content: string;
  type_: string;
  edge_strength: number;
}

interface PathfindingResponse {
  path: PathStep[];
  total_strength: number;
  path_length: number;
  found: boolean;
}

interface KnowledgeAtlasProps {
  className?: string;
  onNodeClick?: (result: SearchResult) => void;
}

const KnowledgeAtlas: React.FC<KnowledgeAtlasProps> = ({ className, onNodeClick }) => {
  const { theme } = useTheme();
  const [nodes, setNodes] = useState<KnowledgeNode[]>([]);
  const [links, setLinks] = useState<any[]>([]);
  const [selectedNode, setSelectedNode] = useState<KnowledgeNode | null>(null);
  const [sourceNode, setSourceNode] = useState<KnowledgeNode | null>(null);
  const [targetNode, setTargetNode] = useState<KnowledgeNode | null>(null);
  const [path, setPath] = useState<PathStep[]>([]);
  const [pathLoading, setPathLoading] = useState(false);
  const [showPathSidebar, setShowPathSidebar] = useState(false);
  const [hoveredNode, setHoveredNode] = useState<KnowledgeNode | null>(null);
  const [pulseTime, setPulseTime] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const fgRef = useRef<any>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerSize, setContainerSize] = useState({ width: 800, height: 600 });
  const [isContainerReady, setIsContainerReady] = useState(false);
  const [shouldRenderGraph, setShouldRenderGraph] = useState(false);
  const [reductionMethod, setReductionMethod] = useState<'pca' | 'umap'>('pca');
  const [maxNodes, setMaxNodes] = useState(500);

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
    // Re-resolve CSS var colors when theme changes
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

  // Update container size
  useEffect(() => {
    const updateSize = () => {
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect();
        const width = Math.max(rect.width || 800, 100);
        const height = Math.max(rect.height || 600, 100);
        if (width > 0 && height > 0) {
          setContainerSize({ width, height });
          setIsContainerReady(true);
        }
      }
    };

    updateSize();
    
    const timeoutId = setTimeout(() => {
      updateSize();
      setIsContainerReady(true);
      setTimeout(() => {
        if (containerRef.current) {
          const rect = containerRef.current.getBoundingClientRect();
          if (rect.width > 0 && rect.height > 0) {
            setShouldRenderGraph(true);
          }
        }
      }, 100);
    }, 200);

    window.addEventListener('resize', updateSize);
    return () => {
      clearTimeout(timeoutId);
      window.removeEventListener('resize', updateSize);
    };
  }, []);

  // Fetch knowledge graph data
  useEffect(() => {
    const fetchKnowledgeGraph = async () => {
      setLoading(true);
      setError(null);
      setGraphError(null);

      try {
        const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';
        const response = await fetch(
          `${orchestratorUrl}/api/knowledge/atlas?method=${reductionMethod}&max_nodes=${maxNodes}`,
          {
            method: 'GET',
            headers: {
              'Accept': 'application/json',
            },
          }
        );

        if (!response.ok) {
          throw new Error(`Failed to fetch knowledge graph: ${response.statusText}`);
        }

        const data = await response.json();
        
        // Transform backend data to graph format
        const graphNodes: KnowledgeNode[] = (data.nodes || []).map((node: any) => ({
          id: node.id,
          x: node.x,
          y: node.y,
          z: node.z,
          title: node.title || 'Untitled',
          type: node.type || 'memory',
          content: node.content || '',
          snippet: node.snippet,
          confidence: node.confidence || 0,
          similarity: node.similarity || 0,
          size: Math.max(2, Math.min(10, (node.confidence || 0) * 10 + 2)),
          color: getNodeColor(node.confidence || 0, node.type || 'memory'),
        }));

        // Transform semantic edges from backend (Neural Filaments)
        const graphLinks: any[] = (data.edges || []).map((edge: KnowledgeEdge) => ({
          source: edge.source,
          target: edge.target,
          value: edge.strength,
          strength: edge.strength,
        }));

        setNodes(graphNodes);
        setLinks(graphLinks);
        setLoading(false);
      } catch (err: any) {
        console.error('[KnowledgeAtlas] Failed to fetch graph:', err);
        setError(err.message || 'Failed to load knowledge graph');
        setGraphError(err.message || 'Failed to load knowledge graph');
        setLoading(false);
      }
    };

    if (isContainerReady) {
      fetchKnowledgeGraph();
    }
  }, [reductionMethod, maxNodes, isContainerReady]);

  // Get node color based on confidence and type
  const getNodeColor = (confidence: number, type: string): string => {
    // High confidence = brighter, low confidence = dimmer
    const intensity = Math.max(0.3, confidence);
    
    switch (type) {
      case 'memory':
        // Gold/amber for memory fragments
        return `rgba(255, 215, 0, ${intensity})`;
      case 'playbook':
        // Blue for playbooks
        return `rgba(59, 130, 246, ${intensity})`;
      case 'chat':
        // Green for chat
        return `rgba(16, 185, 129, ${intensity})`;
      default:
        return `rgba(158, 201, 217, ${intensity})`;
    }
  };

  // Create 3D node object with glow effect for high confidence
  const nodeThreeObject = useCallback((node: any) => {
    const sprite = new THREE.TextureLoader().load('data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==');
    const material = new THREE.SpriteMaterial({
      map: sprite,
      color: node.color || '#9EC9D9',
      transparent: true,
      opacity: Math.max(0.4, node.confidence || 0.5),
    });
    
    const spriteObj = new THREE.Sprite(material);
    spriteObj.scale.set(node.size || 5, node.size || 5, 1);
    
    // Add glow for high confidence nodes
    if (node.confidence > 0.8) {
      const glowMaterial = new THREE.SpriteMaterial({
        map: sprite,
        color: node.color || '#9EC9D9',
        transparent: true,
        opacity: 0.2,
      });
      const glow = new THREE.Sprite(glowMaterial);
      glow.scale.set((node.size || 5) * 2, (node.size || 5) * 2, 1);
      spriteObj.add(glow);
    }
    
    return spriteObj;
  }, []);

  // Track Ctrl key state for path selection
  const [ctrlPressed, setCtrlPressed] = useState(false);
  
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        setCtrlPressed(true);
      }
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      if (!e.ctrlKey && !e.metaKey) {
        setCtrlPressed(false);
      }
    };
    
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);
  
  // Handle node click - support path selection
  const handleNodeClick = useCallback((node: any) => {
    const nodeData = node as KnowledgeNode;
    setSelectedNode(nodeData);
    
    // If Ctrl is pressed and source is set, set as target
    if (ctrlPressed && sourceNode && nodeData.id !== sourceNode.id) {
      setTargetNode(nodeData);
    } else if (!sourceNode) {
      // First click sets source
      setSourceNode(nodeData);
      setTargetNode(null);
      setPath([]);
    } else if (sourceNode.id === nodeData.id) {
      // Clicking source again clears selection
      setSourceNode(null);
      setTargetNode(null);
      setPath([]);
    } else if (!targetNode) {
      // Second click (without Ctrl) sets target
      setTargetNode(nodeData);
    } else {
      // Subsequent clicks update source
      setSourceNode(nodeData);
      setTargetNode(null);
      setPath([]);
    }
    
    if (onNodeClick) {
      const searchResult: SearchResult = {
        id: node.id,
        type: node.type,
        title: node.title,
        content: node.content,
        preview: node.snippet || node.content.substring(0, 200),
        snippet: node.snippet,
        metadata: {
          similarity: node.similarity,
          crossEncoderScore: node.confidence,
          verification_status: node.confidence > 0.8 ? 'High Confidence' : 
                              node.confidence > 0.5 ? 'Medium Confidence' : 'Low Confidence',
        },
      };
      onNodeClick(searchResult);
    }
  }, [onNodeClick, sourceNode]);
  
  // Trace path between source and target
  const handleTracePath = useCallback(async () => {
    if (!sourceNode || !targetNode) return;
    
    setPathLoading(true);
    setShowPathSidebar(true);
    
    try {
      const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';
      const response = await fetch(
        `${orchestratorUrl}/api/knowledge/path`,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Accept': 'application/json',
          },
          body: JSON.stringify({
            source_id: sourceNode.id,
            target_id: targetNode.id,
          }),
        }
      );
      
      if (!response.ok) {
        throw new Error(`Failed to find path: ${response.statusText}`);
      }
      
      const data: PathfindingResponse = await response.json();
      setPath(data.path);
    } catch (err: any) {
      console.error('[KnowledgeAtlas] Failed to trace path:', err);
      setError(err.message || 'Failed to trace path');
      setPath([]);
    } finally {
      setPathLoading(false);
    }
  }, [sourceNode, targetNode]);
  
  // Clear path selection
  const handleClearPath = useCallback(() => {
    setSourceNode(null);
    setTargetNode(null);
    setPath([]);
    setShowPathSidebar(false);
  }, []);

  // Handle node hover (receives node or null)
  const handleNodeHover = useCallback((node: any) => {
    setHoveredNode(node ? (node as KnowledgeNode) : null);
  }, []);

  // Pulse animation for highlighted links
  useEffect(() => {
    if (!hoveredNode) return;
    
    const interval = setInterval(() => {
      setPulseTime(Date.now());
    }, 16); // ~60fps
    
    return () => clearInterval(interval);
  }, [hoveredNode]);

  // Check if a link is connected to the hovered node
  const isLinkHighlighted = useCallback((link: any) => {
    if (!hoveredNode) return false;
    return link.source === hoveredNode.id || link.target === hoveredNode.id;
  }, [hoveredNode]);
  
  // Check if a link is part of the traced path
  const isLinkInPath = useCallback((link: any) => {
    if (path.length < 2) return false;
    for (let i = 0; i < path.length - 1; i++) {
      const currentId = path[i].node_id;
      const nextId = path[i + 1].node_id;
      if ((link.source === currentId && link.target === nextId) ||
          (link.target === currentId && link.source === nextId)) {
        return true;
      }
    }
    return false;
  }, [path]);
  
  // Check if a node is part of the traced path
  const isNodeInPath = useCallback((node: any) => {
    return path.some(step => step.node_id === node.id);
  }, [path]);
  
  // Check if a node is source or target
  const isNodeSourceOrTarget = useCallback((node: any) => {
    return (sourceNode && node.id === sourceNode.id) || 
           (targetNode && node.id === targetNode.id);
  }, [sourceNode, targetNode]);

  // Control functions
  const handleZoomIn = useCallback(() => {
    if (fgRef.current) {
      fgRef.current.zoom(1.5, 500);
    }
  }, []);

  const handleZoomOut = useCallback(() => {
    if (fgRef.current) {
      fgRef.current.zoom(0.67, 500);
    }
  }, []);

  const handleReset = useCallback(() => {
    if (fgRef.current) {
      fgRef.current.zoomToFit(400);
    }
  }, []);

  return (
    <div className={`relative w-full h-full ${className || ''}`}>
      {/* Controls */}
      <div className="absolute top-4 left-4 z-10 flex flex-col gap-2">
        <div className="bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-2 shadow-lg backdrop-blur-sm">
          <div className="flex items-center gap-2 mb-2">
            <Brain className="w-4 h-4 text-[var(--bg-steel)]" />
            <span className="text-xs font-semibold text-[var(--text-primary)]">Knowledge Atlas</span>
          </div>
          <div className="text-[10px] text-[var(--text-secondary)] mb-2 pb-2 border-b border-[rgb(var(--bg-steel-rgb)/0.2)]">
            <div className="mb-1">💡 <strong>Trace Path:</strong> Click node to select source, then click another node to set target. Press <strong>Trace Path</strong> to find the semantic connection.</div>
          </div>
          
          <div className="flex flex-col gap-2 text-xs">
            <div className="flex items-center gap-2">
              <label className="text-[var(--text-secondary)]">Method:</label>
              <select
                value={reductionMethod}
                onChange={(e) => setReductionMethod(e.target.value as 'pca' | 'umap')}
                className="bg-[rgb(var(--bg-muted-rgb)/0.5)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded px-2 py-1 text-[var(--text-primary)] text-xs"
              >
                <option value="pca">PCA</option>
                <option value="umap">UMAP</option>
              </select>
            </div>
            
            <div className="flex items-center gap-2">
              <label className="text-[var(--text-secondary)]">Max Nodes:</label>
              <input
                type="number"
                value={maxNodes}
                onChange={(e) => setMaxNodes(Math.max(100, Math.min(2000, parseInt(e.target.value) || 500)))}
                className="w-20 bg-[rgb(var(--bg-muted-rgb)/0.5)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded px-2 py-1 text-[var(--text-primary)] text-xs"
                min="100"
                max="2000"
              />
            </div>
            
            {/* Path Selection Info */}
            {(sourceNode || targetNode) && (
              <div className="mt-2 pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                <div className="text-[var(--text-secondary)] text-[10px] mb-1">Path Selection:</div>
                {sourceNode && (
                  <div className="text-[10px] text-[var(--text-primary)]">
                    Source: <span className="font-semibold text-[#FF1493]">{sourceNode.title.substring(0, 20)}...</span>
                  </div>
                )}
                {targetNode && (
                  <div className="text-[10px] text-[var(--text-primary)]">
                    Target: <span className="font-semibold text-[#FF1493]">{targetNode.title.substring(0, 20)}...</span>
                  </div>
                )}
                {sourceNode && targetNode && (
                  <button
                    onClick={handleTracePath}
                    disabled={pathLoading}
                    className="mt-2 w-full px-2 py-1 bg-[#FF1493] hover:bg-[#FF69B4] disabled:bg-[rgb(var(--bg-muted-rgb)/0.5)] text-white text-[10px] rounded transition-colors flex items-center justify-center gap-1"
                  >
                    {pathLoading ? (
                      <>
                        <div className="animate-spin rounded-full h-3 w-3 border-b border-white"></div>
                        <span>Tracing...</span>
                      </>
                    ) : (
                      <>
                        <Route className="w-3 h-3" />
                        <span>Trace Path</span>
                      </>
                    )}
                  </button>
                )}
                {(sourceNode || targetNode) && (
                  <button
                    onClick={handleClearPath}
                    className="mt-1 w-full px-2 py-1 bg-[rgb(var(--bg-muted-rgb)/0.5)] hover:bg-[rgb(var(--bg-muted-rgb)/0.7)] text-[var(--text-secondary)] text-[10px] rounded transition-colors"
                  >
                    Clear
                  </button>
                )}
              </div>
            )}
          </div>
        </div>

        <div className="flex gap-2">
          <button
            onClick={handleZoomIn}
            className="p-2 bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded hover:bg-[rgb(var(--bg-muted-rgb)/0.5)] transition-colors"
            title="Zoom In"
          >
            <ZoomIn className="w-4 h-4 text-[var(--text-primary)]" />
          </button>
          <button
            onClick={handleZoomOut}
            className="p-2 bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded hover:bg-[rgb(var(--bg-muted-rgb)/0.5)] transition-colors"
            title="Zoom Out"
          >
            <ZoomOut className="w-4 h-4 text-[var(--text-primary)]" />
          </button>
          <button
            onClick={handleReset}
            className="p-2 bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded hover:bg-[rgb(var(--bg-muted-rgb)/0.5)] transition-colors"
            title="Reset View"
          >
            <RotateCcw className="w-4 h-4 text-[var(--text-primary)]" />
          </button>
        </div>
      </div>

      {/* Legend */}
      <div className="absolute top-4 right-4 z-10 bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-3 shadow-lg backdrop-blur-sm">
        <div className="text-xs font-semibold text-[var(--text-primary)] mb-2">Legend</div>
        <div className="space-y-1.5 text-xs">
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-[rgba(255,215,0,0.8)]"></div>
            <span className="text-[var(--text-secondary)]">Memory</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-[rgba(59,130,246,0.8)]"></div>
            <span className="text-[var(--text-secondary)]">Playbook</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-[rgba(16,185,129,0.8)]"></div>
            <span className="text-[var(--text-secondary)]">Chat</span>
          </div>
          <div className="mt-2 pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
            <div className="text-[var(--text-secondary)] text-[10px]">
              Brightness = Confidence
            </div>
          </div>
        </div>
      </div>

      {/* Graph Container */}
      <div ref={containerRef} className="w-full h-full">
        {loading && (
          <div className="absolute inset-0 flex items-center justify-center bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm z-20">
            <div className="text-center">
              <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-[var(--bg-steel)] mb-2"></div>
              <p className="text-sm text-[var(--text-secondary)]">Loading knowledge graph...</p>
            </div>
          </div>
        )}

        {error && (
          <div className="absolute inset-0 flex items-center justify-center bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm z-20">
            <div className="text-center p-4 bg-[rgb(var(--bg-muted-rgb)/0.5)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)]">
              <Info className="w-6 h-6 text-[var(--bg-steel)] mx-auto mb-2" />
              <p className="text-sm text-[var(--text-primary)] mb-2">{error}</p>
              <button
                onClick={() => window.location.reload()}
                className="text-xs text-[var(--bg-steel)] hover:underline"
              >
                Retry
              </button>
            </div>
          </div>
        )}

        {(containerSize.width > 0 && containerSize.height > 0 && isContainerReady && shouldRenderGraph && !graphError) && (
          <ForceGraph3D
            ref={fgRef}
            graphData={{ nodes, links }}
            nodeLabel={(node: any) => `${node.title}\nConfidence: ${((node.confidence || 0) * 100).toFixed(1)}%`}
            nodeColor={(node: any) => {
              // Highlight source and target nodes
              if (isNodeSourceOrTarget(node)) {
                return '#FF1493'; // Neon pink for source/target
              }
              // Highlight nodes in path
              if (isNodeInPath(node)) {
                return '#FF69B4'; // Lighter pink for path nodes
              }
              return node.color || '#9EC9D9';
            }}
            nodeThreeObject={nodeThreeObject}
            linkColor={(link: any) => {
              // Priority: Path > Hovered > Default
              if (isLinkInPath(link)) {
                // Neon pink for path links with pulse
                const pulseIntensity = 0.7 + 0.3 * Math.sin(pulseTime / 150);
                return `rgba(255, 20, 147, ${pulseIntensity})`; // Neon pink (DeepPink)
              }
              // Highlight links connected to hovered node with pulse effect
              if (isLinkHighlighted(link)) {
                const pulseIntensity = 0.5 + 0.3 * Math.sin(pulseTime / 200); // Pulse animation
                return `rgba(100, 200, 255, ${0.6 + pulseIntensity * 0.4})`; // Bright blue glow
              }
              // Default: faint glowing Neural Filament
              const strength = link.strength || link.value || 0.85;
              return `rgba(158, 201, 217, ${0.2 + strength * 0.3})`; // Glow intensity based on strength
            }}
            linkWidth={(link: any) => {
              // Thickest for path links
              if (isLinkInPath(link)) {
                return 3.0;
              }
              // Thicker links for highlighted connections
              if (isLinkHighlighted(link)) {
                return 2.0;
              }
              // Base width with slight variation based on strength
              const strength = link.strength || link.value || 0.85;
              return 0.5 + strength * 0.5;
            }}
            linkOpacity={(link: any) => {
              // Most opaque for path links
              if (isLinkInPath(link)) {
                return 1.0;
              }
              // More opaque for highlighted links
              if (isLinkHighlighted(link)) {
                return 0.9;
              }
              // Base opacity with strength-based variation
              const strength = link.strength || link.value || 0.85;
              return 0.3 + strength * 0.4;
            }}
            onNodeClick={handleNodeClick}
            onNodeHover={handleNodeHover}
            backgroundColor={themeColors.overlaySolid}
            showNavInfo={false}
            width={containerSize.width}
            height={containerSize.height}
            cooldownTicks={100}
            onEngineStop={() => {
              if (fgRef.current) {
                fgRef.current.zoomToFit(400);
              }
            }}
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
      </div>

      {/* Path Reasoning Sidebar */}
      {showPathSidebar && path.length > 0 && (
        <div className="absolute right-4 top-4 bottom-4 w-96 z-10 bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg shadow-lg backdrop-blur-sm flex flex-col">
          <div className="flex items-center justify-between p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)]">
            <div className="flex items-center gap-2">
              <Sparkles className="w-5 h-5 text-[#FF1493]" />
              <h3 className="font-bold text-[var(--text-primary)]">Neural Path</h3>
            </div>
            <button
              onClick={() => setShowPathSidebar(false)}
              className="p-1 hover:bg-[rgb(var(--bg-muted-rgb)/0.5)] rounded transition-colors"
            >
              <X className="w-4 h-4 text-[var(--text-secondary)]" />
            </button>
          </div>
          
          <div className="flex-1 overflow-y-auto p-4 space-y-4">
            <div className="text-xs text-[var(--text-secondary)] mb-2">
              Path Length: <span className="font-semibold text-[var(--text-primary)]">{path.length} steps</span>
            </div>
            
            {path.map((step, index) => (
              <div
                key={step.node_id}
                className="bg-[rgb(var(--bg-muted-rgb)/0.3)] rounded-lg p-3 border-l-4 border-[#FF1493]"
              >
                <div className="flex items-start justify-between mb-2">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs font-bold text-[#FF1493]">#{index + 1}</span>
                      <span className="text-xs font-semibold text-[var(--text-primary)]">{step.title}</span>
                    </div>
                    <div className="text-[10px] text-[var(--text-secondary)] mb-2">
                      Type: <span className="font-semibold">{step.type_}</span>
                      {index > 0 && (
                        <span className="ml-2">
                          Edge Strength: <span className="font-semibold text-[#FF1493]">{(step.edge_strength * 100).toFixed(1)}%</span>
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                {step.snippet && (
                  <div className="text-xs text-[var(--text-primary)] line-clamp-3 mt-2 pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                    {step.snippet}
                  </div>
                )}
                {index < path.length - 1 && (
                  <div className="flex justify-center mt-2 pt-2">
                    <div className="w-0.5 h-4 bg-[#FF1493]"></div>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Node Details Panel */}
      {selectedNode && (
        <div className="absolute bottom-4 left-4 right-4 z-10 bg-[rgb(var(--surface-rgb)/0.95)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4 shadow-lg backdrop-blur-sm max-h-64 overflow-y-auto">
          <div className="flex items-start justify-between mb-2">
            <h3 className="font-bold text-[var(--text-primary)]">{selectedNode.title}</h3>
            <button
              onClick={() => setSelectedNode(null)}
              className="p-1 hover:bg-[rgb(var(--bg-muted-rgb)/0.5)] rounded transition-colors"
            >
              <X className="w-4 h-4 text-[var(--text-secondary)]" />
            </button>
          </div>
          <div className="text-xs text-[var(--text-secondary)] space-y-1">
            <div>Type: <span className="font-semibold">{selectedNode.type}</span></div>
            <div>Confidence: <span className="font-semibold">{((selectedNode.confidence || 0) * 100).toFixed(1)}%</span></div>
            <div>Similarity: <span className="font-semibold">{((selectedNode.similarity || 0) * 100).toFixed(1)}%</span></div>
            {selectedNode.snippet && (
              <div className="mt-2 pt-2 border-t border-[rgb(var(--bg-steel-rgb)/0.2)]">
                <p className="text-[var(--text-primary)] line-clamp-3">{selectedNode.snippet}</p>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default KnowledgeAtlas;
