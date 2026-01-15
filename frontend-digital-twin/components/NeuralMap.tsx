import React, { useRef, useEffect } from 'react';
import KnowledgeAtlas from './KnowledgeAtlas';
import PhoenixMemoryFlow from './PhoenixMemoryFlow';
import CapabilityHeatmap from './CapabilityHeatmap';
import ToolSimulator from './ToolSimulator';
import PlaybookLibrary from './PlaybookLibrary';
import { SearchResult } from '../services/phoenixSearchService';

interface NeuralMapProps {
  view: 'atlas' | 'flow' | 'heatmap' | 'simulator' | 'playbooks';
  selectedMemoryNodeId?: string | null;
  onMemoryNodeClick?: (nodeId: string) => void;
}

const NeuralMap: React.FC<NeuralMapProps> = ({ view, selectedMemoryNodeId, onMemoryNodeClick }) => {
  const atlasRef = useRef<{ centerOnNode?: (nodeId: string) => void } | null>(null);

  // Center on selected memory node when it changes
  useEffect(() => {
    if (selectedMemoryNodeId && view === 'atlas' && atlasRef.current?.centerOnNode) {
      // Small delay to ensure the graph is rendered
      setTimeout(() => {
        atlasRef.current?.centerOnNode?.(selectedMemoryNodeId);
      }, 100);
    }
  }, [selectedMemoryNodeId, view]);

  const handleNodeClick = (result: SearchResult) => {
    onMemoryNodeClick?.(result.id);
  };

  return (
    <div className="min-h-0 bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl overflow-hidden h-full flex flex-col">
      <div className="h-full min-h-0 p-3">
        {view === 'atlas' ? (
          <KnowledgeAtlas className="h-full" onNodeClick={handleNodeClick} />
        ) : view === 'flow' ? (
          <PhoenixMemoryFlow />
        ) : view === 'heatmap' ? (
          <CapabilityHeatmap className="h-full" />
        ) : view === 'simulator' ? (
          <ToolSimulator className="h-full" />
        ) : view === 'playbooks' ? (
          <PlaybookLibrary className="h-full" />
        ) : (
          <KnowledgeAtlas className="h-full" onNodeClick={handleNodeClick} />
        )}
      </div>
    </div>
  );
};

export default NeuralMap;
