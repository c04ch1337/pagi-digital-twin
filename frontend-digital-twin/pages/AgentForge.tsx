import React, { useState, useEffect } from 'react';
import { X, Save, Play, Plus, Trash2, Settings } from 'lucide-react';

interface AgentManifest {
  name: string;
  role: string;
  category: string;
  base_prompt: string;
  tools: string[];
}

interface Tool {
  id: string;
  name: string;
  description: string;
}

interface AgentForgeProps {
  onClose: () => void;
}

const AgentForge: React.FC<AgentForgeProps> = ({ onClose }) => {
  const [manifest, setManifest] = useState<AgentManifest>({
    name: '',
    role: '',
    category: 'tactical',
    base_prompt: '',
    tools: [],
  });

  const [availableTools, setAvailableTools] = useState<Tool[]>([]);
  const [testPrompt, setTestPrompt] = useState('');
  const [testResponse, setTestResponse] = useState('');
  const [isTesting, setIsTesting] = useState(false);
  const [isForging, setIsForging] = useState(false);

  // Fetch available tools from the ToolRegistry
  useEffect(() => {
    const fetchTools = async () => {
      try {
        const response = await fetch('http://localhost:8080/api/tools/list');
        if (response.ok) {
          const tools = await response.json();
          setAvailableTools(tools);
        }
      } catch (error) {
        console.error('[AgentForge] Failed to fetch tools:', error);
      }
    };
    fetchTools();
  }, []);

  const handleTestPrompt = async () => {
    if (!testPrompt.trim()) return;
    
    setIsTesting(true);
    try {
      // Send test prompt to backend with current manifest settings
      const response = await fetch('http://localhost:8080/api/forge/test-prompt', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          system_prompt: manifest.base_prompt,
          user_prompt: testPrompt,
        }),
      });

      if (response.ok) {
        const data = await response.json();
        setTestResponse(data.response);
      } else {
        setTestResponse('Error: Failed to test prompt');
      }
    } catch (error) {
      console.error('[AgentForge] Test prompt failed:', error);
      setTestResponse('Error: Connection failed');
    } finally {
      setIsTesting(false);
    }
  };

  const handleForgeAgent = async () => {
    if (!manifest.name || !manifest.role || !manifest.base_prompt) {
      alert('Please fill in all required fields (Name, Role, Base Prompt)');
      return;
    }

    setIsForging(true);
    try {
      // Send manifest to backend to create agent
      const response = await fetch('http://localhost:8080/api/forge/agent', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(manifest),
      });

      if (response.ok) {
        alert('Agent forged successfully! Check the agent-templates directory.');
        onClose();
      } else {
        const error = await response.text();
        alert(`Failed to forge agent: ${error}`);
      }
    } catch (error) {
      console.error('[AgentForge] Forge failed:', error);
      alert('Failed to forge agent: Connection error');
    } finally {
      setIsForging(false);
    }
  };

  const toggleTool = (toolId: string) => {
    setManifest(prev => ({
      ...prev,
      tools: prev.tools.includes(toolId)
        ? prev.tools.filter(t => t !== toolId)
        : [...prev.tools, toolId],
    }));
  };

  return (
    <div className="h-full flex flex-col bg-[var(--bg-primary)]">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-[var(--bg-steel)] to-[var(--bg-muted)] flex items-center justify-center">
            <Settings className="w-6 h-6 text-[var(--text-on-accent)]" />
          </div>
          <div>
            <h2 className="text-lg font-bold text-[var(--text-primary)]">AgentForge</h2>
            <p className="text-xs text-[rgb(var(--text-primary-rgb)/0.7)]">Visual Agent Assembly & DNA Configuration</p>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-2 hover:bg-[var(--bg-muted)] rounded-lg transition-colors"
        >
          <X className="w-5 h-5 text-[var(--text-primary)]" />
        </button>
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-6xl mx-auto grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Left Column: Agent Configuration */}
          <div className="space-y-6">
            {/* Basic Info */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-steel)]"></div>
                Agent Identity
              </h3>
              <div className="space-y-4">
                <div>
                  <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                    Agent Name *
                  </label>
                  <input
                    type="text"
                    value={manifest.name}
                    onChange={(e) => setManifest({ ...manifest, name: e.target.value })}
                    placeholder="e.g., SecurityAnalyst"
                    className="w-full px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                    Role *
                  </label>
                  <input
                    type="text"
                    value={manifest.role}
                    onChange={(e) => setManifest({ ...manifest, role: e.target.value })}
                    placeholder="e.g., Threat Detection Specialist"
                    className="w-full px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                    Category
                  </label>
                  <select
                    value={manifest.category}
                    onChange={(e) => setManifest({ ...manifest, category: e.target.value })}
                    className="w-full px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                  >
                    <option value="tactical">Tactical</option>
                    <option value="strategic">Strategic</option>
                    <option value="operational">Operational</option>
                    <option value="research">Research</option>
                  </select>
                </div>
              </div>
            </div>

            {/* System Prompt */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-steel)]"></div>
                Base Prompt (Agent DNA) *
              </h3>
              <textarea
                value={manifest.base_prompt}
                onChange={(e) => setManifest({ ...manifest, base_prompt: e.target.value })}
                placeholder="Define the agent's core behavior, expertise, and decision-making framework..."
                className="w-full h-48 px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] font-mono resize-none"
              />
            </div>

            {/* Tool Mapper */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-steel)]"></div>
                Tool Capabilities ({manifest.tools.length} selected)
              </h3>
              <div className="space-y-2 max-h-64 overflow-y-auto">
                {availableTools.length === 0 ? (
                  <p className="text-xs text-[rgb(var(--text-primary-rgb)/0.5)] italic">Loading tools...</p>
                ) : (
                  availableTools.map((tool) => (
                    <label
                      key={tool.id}
                      className="flex items-start gap-3 p-3 bg-[rgb(var(--surface-rgb)/0.5)] rounded-lg hover:bg-[rgb(var(--surface-rgb)/0.8)] cursor-pointer transition-colors"
                    >
                      <input
                        type="checkbox"
                        checked={manifest.tools.includes(tool.id)}
                        onChange={() => toggleTool(tool.id)}
                        className="mt-0.5"
                      />
                      <div className="flex-1">
                        <div className="text-sm font-medium text-[var(--text-primary)]">{tool.name}</div>
                        <div className="text-xs text-[rgb(var(--text-primary-rgb)/0.6)]">{tool.description}</div>
                      </div>
                    </label>
                  ))
                )}
              </div>
            </div>
          </div>

          {/* Right Column: Prompt Lab */}
          <div className="space-y-6">
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-muted)]"></div>
                Prompt Lab (Test Agent Behavior)
              </h3>
              <div className="space-y-4">
                <div>
                  <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                    Test Input
                  </label>
                  <textarea
                    value={testPrompt}
                    onChange={(e) => setTestPrompt(e.target.value)}
                    placeholder="Enter a test scenario to see how your agent responds..."
                    className="w-full h-32 px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] resize-none"
                  />
                </div>
                <button
                  onClick={handleTestPrompt}
                  disabled={isTesting || !testPrompt.trim() || !manifest.base_prompt}
                  className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-[var(--bg-muted)] hover:bg-[var(--bg-steel)] disabled:bg-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-on-accent)] rounded-lg transition-colors text-sm font-medium"
                >
                  <Play className="w-4 h-4" />
                  {isTesting ? 'Testing...' : 'Test Prompt'}
                </button>
                {testResponse && (
                  <div>
                    <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                      Agent Response
                    </label>
                    <div className="p-4 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm whitespace-pre-wrap max-h-64 overflow-y-auto">
                      {testResponse}
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* Preview */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-muted)]"></div>
                Manifest Preview
              </h3>
              <pre className="text-xs bg-[var(--text-primary)] text-[var(--bg-secondary)] p-4 rounded-lg overflow-x-auto font-mono">
                {JSON.stringify(manifest, null, 2)}
              </pre>
            </div>
          </div>
        </div>
      </div>

      {/* Footer Actions */}
      <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] p-4 bg-[var(--bg-secondary)] flex items-center justify-between">
        <div className="text-xs text-[rgb(var(--text-primary-rgb)/0.7)]">
          * Required fields must be filled before forging
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-[rgb(var(--surface-rgb)/0.5)] hover:bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] rounded-lg transition-colors text-sm font-medium"
          >
            Cancel
          </button>
          <button
            onClick={handleForgeAgent}
            disabled={isForging || !manifest.name || !manifest.role || !manifest.base_prompt}
            className="flex items-center gap-2 px-6 py-2 bg-gradient-to-r from-[var(--bg-steel)] to-[var(--bg-muted)] hover:from-[var(--bg-muted)] hover:to-[var(--bg-steel)] disabled:from-[rgb(var(--bg-steel-rgb)/0.3)] disabled:to-[rgb(var(--bg-muted-rgb)/0.3)] text-[var(--text-on-accent)] rounded-lg transition-all text-sm font-bold shadow-lg"
          >
            <Save className="w-4 h-4" />
            {isForging ? 'Forging...' : 'Forge Agent'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default AgentForge;
