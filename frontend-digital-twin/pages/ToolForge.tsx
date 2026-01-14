import React, { useState, useEffect } from 'react';
import { X, Save, Play, Terminal, Shield, Globe, FileText, Code } from 'lucide-react';
import Editor from '@monaco-editor/react';

interface ToolMetadata {
  name: string;
  description: string;
  language: 'python' | 'rust';
  sudo_required: boolean;
  network_access: boolean;
  file_write: boolean;
}

interface ToolForgeProps {
  onClose: () => void;
}

const ToolForge: React.FC<ToolForgeProps> = ({ onClose }) => {
  const [metadata, setMetadata] = useState<ToolMetadata>({
    name: '',
    description: '',
    language: 'python',
    sudo_required: false,
    network_access: false,
    file_write: false,
  });

  const [code, setCode] = useState<string>('# Write your tool code here\n\ndef execute():\n    """Main entry point for the tool"""\n    pass\n');
  const [testOutput, setTestOutput] = useState<string>('');
  const [isTesting, setIsTesting] = useState(false);
  const [isForging, setIsForging] = useState(false);

  const handleTestRun = async () => {
    if (!code.trim()) {
      setTestOutput('Error: No code to test');
      return;
    }

    setIsTesting(true);
    setTestOutput('Executing in sandbox...\n');

    try {
      const response = await fetch('http://localhost:8080/api/forge/test-tool', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          code,
          language: metadata.language,
          permissions: {
            sudo: metadata.sudo_required,
            network: metadata.network_access,
            file_write: metadata.file_write,
          },
        }),
      });

      if (response.ok) {
        const data = await response.json();
        setTestOutput(prev => prev + '\n' + data.stdout + '\n' + (data.stderr || ''));
      } else {
        const error = await response.text();
        setTestOutput(prev => prev + '\nError: ' + error);
      }
    } catch (error) {
      console.error('[ToolForge] Test run failed:', error);
      setTestOutput(prev => prev + '\nError: Connection failed');
    } finally {
      setIsTesting(false);
    }
  };

  const handleForgeTool = async () => {
    if (!metadata.name || !metadata.description || !code.trim()) {
      alert('Please fill in all required fields (Name, Description, Code)');
      return;
    }

    setIsForging(true);
    try {
      // Send tool to backend for privacy scrubbing and commit
      const response = await fetch('http://localhost:8080/api/forge/tool', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          metadata,
          code,
        }),
      });

      if (response.ok) {
        alert('Tool forged successfully! It has been committed to the tool repository.');
        onClose();
      } else {
        const error = await response.text();
        alert(`Failed to forge tool: ${error}`);
      }
    } catch (error) {
      console.error('[ToolForge] Forge failed:', error);
      alert('Failed to forge tool: Connection error');
    } finally {
      setIsForging(false);
    }
  };

  const getEditorLanguage = () => {
    return metadata.language === 'rust' ? 'rust' : 'python';
  };

  const getDefaultCode = (lang: 'python' | 'rust') => {
    if (lang === 'rust') {
      return `// Write your tool code here\n\npub fn execute() -> Result<(), Box<dyn std::error::Error>> {\n    // Tool implementation\n    Ok(())\n}\n`;
    }
    return `# Write your tool code here\n\ndef execute():\n    """Main entry point for the tool"""\n    pass\n`;
  };

  useEffect(() => {
    setCode(getDefaultCode(metadata.language));
  }, [metadata.language]);

  return (
    <div className="h-full flex flex-col bg-[var(--bg-primary)]">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.3)] bg-[var(--bg-secondary)]">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-[var(--bg-muted)] to-[var(--bg-steel)] flex items-center justify-center">
            <Code className="w-6 h-6 text-[var(--text-on-accent)]" />
          </div>
          <div>
            <h2 className="text-lg font-bold text-[var(--text-primary)]">ToolForge</h2>
            <p className="text-xs text-[rgb(var(--text-primary-rgb)/0.7)]">Dynamic Tool Development & Sandbox Testing</p>
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
        <div className="max-w-7xl mx-auto grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Left Column: Metadata & Permissions */}
          <div className="space-y-6">
            {/* Tool Metadata */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-steel)]"></div>
                Tool Metadata
              </h3>
              <div className="space-y-4">
                <div>
                  <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                    Tool Name *
                  </label>
                  <input
                    type="text"
                    value={metadata.name}
                    onChange={(e) => setMetadata({ ...metadata, name: e.target.value })}
                    placeholder="e.g., network_scanner"
                    className="w-full px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                    Description *
                  </label>
                  <textarea
                    value={metadata.description}
                    onChange={(e) => setMetadata({ ...metadata, description: e.target.value })}
                    placeholder="Describe what this tool does..."
                    className="w-full h-24 px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)] resize-none"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-[rgb(var(--text-primary-rgb)/0.7)] mb-1">
                    Language
                  </label>
                  <select
                    value={metadata.language}
                    onChange={(e) => setMetadata({ ...metadata, language: e.target.value as 'python' | 'rust' })}
                    className="w-full px-3 py-2 bg-[rgb(var(--surface-rgb)/1)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[var(--bg-steel)]"
                  >
                    <option value="python">Python</option>
                    <option value="rust">Rust</option>
                  </select>
                </div>
              </div>
            </div>

            {/* Permission Toggles */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-muted)]"></div>
                Security Permissions
              </h3>
              <div className="space-y-3">
                <label className="flex items-center gap-3 p-3 bg-[rgb(var(--surface-rgb)/0.5)] rounded-lg hover:bg-[rgb(var(--surface-rgb)/0.8)] cursor-pointer transition-colors">
                  <input
                    type="checkbox"
                    checked={metadata.sudo_required}
                    onChange={(e) => setMetadata({ ...metadata, sudo_required: e.target.checked })}
                    className="w-4 h-4"
                  />
                  <Shield className="w-4 h-4 text-[var(--bg-steel)]" />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-[var(--text-primary)]">Sudo Required</div>
                    <div className="text-xs text-[rgb(var(--text-primary-rgb)/0.6)]">Elevated system privileges</div>
                  </div>
                </label>

                <label className="flex items-center gap-3 p-3 bg-[rgb(var(--surface-rgb)/0.5)] rounded-lg hover:bg-[rgb(var(--surface-rgb)/0.8)] cursor-pointer transition-colors">
                  <input
                    type="checkbox"
                    checked={metadata.network_access}
                    onChange={(e) => setMetadata({ ...metadata, network_access: e.target.checked })}
                    className="w-4 h-4"
                  />
                  <Globe className="w-4 h-4 text-[var(--bg-steel)]" />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-[var(--text-primary)]">Network Access</div>
                    <div className="text-xs text-[rgb(var(--text-primary-rgb)/0.6)]">Internet connectivity</div>
                  </div>
                </label>

                <label className="flex items-center gap-3 p-3 bg-[rgb(var(--surface-rgb)/0.5)] rounded-lg hover:bg-[rgb(var(--surface-rgb)/0.8)] cursor-pointer transition-colors">
                  <input
                    type="checkbox"
                    checked={metadata.file_write}
                    onChange={(e) => setMetadata({ ...metadata, file_write: e.target.checked })}
                    className="w-4 h-4"
                  />
                  <FileText className="w-4 h-4 text-[var(--bg-steel)]" />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-[var(--text-primary)]">File Write</div>
                    <div className="text-xs text-[rgb(var(--text-primary-rgb)/0.6)]">Modify filesystem</div>
                  </div>
                </label>
              </div>
            </div>

            {/* Metadata Preview */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg p-6 border border-[rgb(var(--bg-steel-rgb)/0.2)]">
              <h3 className="text-sm font-bold text-[var(--text-primary)] mb-4 flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-[var(--bg-muted)]"></div>
                Tool Manifest
              </h3>
              <pre className="text-xs bg-[var(--text-primary)] text-[var(--bg-secondary)] p-4 rounded-lg overflow-x-auto font-mono">
                {JSON.stringify(metadata, null, 2)}
              </pre>
            </div>
          </div>

          {/* Middle Column: Code Editor */}
          <div className="lg:col-span-2 space-y-6">
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.2)] overflow-hidden">
              <div className="flex items-center justify-between p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)] bg-[rgb(var(--surface-rgb)/0.5)]">
                <h3 className="text-sm font-bold text-[var(--text-primary)] flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-[var(--bg-steel)]"></div>
                  Code Editor ({metadata.language})
                </h3>
                <button
                  onClick={handleTestRun}
                  disabled={isTesting || !code.trim()}
                  className="flex items-center gap-2 px-4 py-2 bg-[var(--bg-muted)] hover:bg-[var(--bg-steel)] disabled:bg-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--text-on-accent)] rounded-lg transition-colors text-sm font-medium"
                >
                  <Play className="w-4 h-4" />
                  {isTesting ? 'Testing...' : 'Test Run'}
                </button>
              </div>
              <div className="h-[500px]">
                <Editor
                  height="100%"
                  language={getEditorLanguage()}
                  value={code}
                  onChange={(value) => setCode(value || '')}
                  theme="vs-dark"
                  options={{
                    minimap: { enabled: false },
                    fontSize: 14,
                    lineNumbers: 'on',
                    scrollBeyondLastLine: false,
                    automaticLayout: true,
                    tabSize: 4,
                  }}
                />
              </div>
            </div>

            {/* Console Output */}
            <div className="bg-[rgb(var(--surface-rgb)/0.8)] backdrop-blur-sm rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.2)] overflow-hidden">
              <div className="flex items-center gap-2 p-4 border-b border-[rgb(var(--bg-steel-rgb)/0.2)] bg-[rgb(var(--surface-rgb)/0.5)]">
                <Terminal className="w-4 h-4 text-[var(--bg-steel)]" />
                <h3 className="text-sm font-bold text-[var(--text-primary)]">
                  Sandbox Console
                </h3>
              </div>
              <div className="p-4 bg-[var(--text-primary)] text-[var(--bg-secondary)] font-mono text-xs h-64 overflow-y-auto">
                {testOutput || 'No output yet. Click "Test Run" to execute your tool in the sandbox.'}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Footer Actions */}
      <div className="border-t border-[rgb(var(--bg-steel-rgb)/0.3)] p-4 bg-[var(--bg-secondary)] flex items-center justify-between">
        <div className="text-xs text-[rgb(var(--text-primary-rgb)/0.7)]">
          * Tool will be privacy-scrubbed before commit
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-[rgb(var(--surface-rgb)/0.5)] hover:bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-primary)] rounded-lg transition-colors text-sm font-medium"
          >
            Cancel
          </button>
          <button
            onClick={handleForgeTool}
            disabled={isForging || !metadata.name || !metadata.description || !code.trim()}
            className="flex items-center gap-2 px-6 py-2 bg-gradient-to-r from-[var(--bg-muted)] to-[var(--bg-steel)] hover:from-[var(--bg-steel)] hover:to-[var(--bg-muted)] disabled:from-[rgb(var(--bg-steel-rgb)/0.3)] disabled:to-[rgb(var(--bg-muted-rgb)/0.3)] text-[var(--text-on-accent)] rounded-lg transition-all text-sm font-bold shadow-lg"
          >
            <Save className="w-4 h-4" />
            {isForging ? 'Forging...' : 'Commit to Forge'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ToolForge;
