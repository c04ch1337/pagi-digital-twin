import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from './ui/card';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { Textarea } from './ui/textarea';
import { Badge } from './ui/badge';
import { ScrollArea } from './ui/scroll-area';
import { Alert, AlertDescription } from './ui/alert';
import {
  Save,
  RefreshCw,
  AlertTriangle,
  CheckCircle,
  Package,
  GitBranch,
  AlertCircle,
  ArrowUpCircle
} from 'lucide-react';

interface Tool {
  id: string;
  name: string;
  description: string;
  version: string;
  inputSchema: any;
  script: string;
  status: 'active' | 'legacy' | 'deprecated';
  dependentAgents: string[];
  breakingChanges: boolean;
}

interface ToolForgeProps {
  className?: string;
}

export default function ToolForge({ className }: ToolForgeProps) {
  const [tools, setTools] = useState<Tool[]>([]);
  const [selectedTool, setSelectedTool] = useState<Tool | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    fetchTools();
  }, []);

  const fetchTools = async () => {
    setLoading(true);
    try {
      const response = await fetch('/api/tools');
      const data = await response.json();
      setTools(data);
      if (data.length > 0 && !selectedTool) {
        setSelectedTool(data[0]);
      }
    } catch (error) {
      console.error('Failed to fetch tools:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!selectedTool) return;

    setSaving(true);
    try {
      const response = await fetch(`/api/tools/${selectedTool.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(selectedTool),
      });

      if (response.ok) {
        const result = await response.json();
        
        // Check for breaking changes
        if (result.breakingChanges && result.affectedAgents.length > 0) {
          alert(
            `⚠️ Breaking Change Detected!\n\n` +
            `This tool update affects ${result.affectedAgents.length} agent(s):\n` +
            result.affectedAgents.join(', ') +
            `\n\nPlease update these agents in AgentForge.`
          );
        }

        await fetchTools();
      }
    } catch (error) {
      console.error('Failed to save tool:', error);
    } finally {
      setSaving(false);
    }
  };

  const handleFieldChange = (field: keyof Tool, value: any) => {
    if (!selectedTool) return;
    setSelectedTool({ ...selectedTool, [field]: value });
  };

  const handleMarkLegacy = async () => {
    if (!selectedTool) return;

    try {
      await fetch(`/api/tools/${selectedTool.id}/mark-legacy`, {
        method: 'POST',
      });
      await fetchTools();
    } catch (error) {
      console.error('Failed to mark as legacy:', error);
    }
  };

  const getVersionBadge = (version: string) => {
    const [major, minor, patch] = version.split('.').map(Number);
    return (
      <Badge variant="outline" className="font-mono">
        <GitBranch className="w-3 h-3 mr-1" />
        v{version}
      </Badge>
    );
  };

  const getStatusBadge = (status: string) => {
    const variants: Record<string, any> = {
      active: 'default',
      legacy: 'secondary',
      deprecated: 'destructive',
    };
    return (
      <Badge variant={variants[status] || 'default'}>
        {status}
      </Badge>
    );
  };

  return (
    <div className={`flex flex-col h-full ${className}`}>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-2xl font-bold text-[var(--text-on-accent)]">ToolForge</h2>
          <p className="text-sm text-[var(--text-muted)]">Technical Excellence Layer: Build & Version Tools</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={fetchTools}
            disabled={loading}
          >
            <RefreshCw className={`w-4 h-4 mr-2 ${loading ? 'animate-spin' : ''}`} />
            Refresh
          </Button>
          <Button
            size="sm"
            onClick={handleSave}
            disabled={!selectedTool || saving}
          >
            <Save className="w-4 h-4 mr-2" />
            {saving ? 'Saving...' : 'Save & Version'}
          </Button>
        </div>
      </div>

      <div className="flex gap-4 flex-1 overflow-hidden">
        {/* Tool List Sidebar */}
        <Card className="w-64 bg-[var(--bg-secondary)] border-[var(--border-color)]">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">Tools</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <ScrollArea className="h-[calc(100vh-250px)]">
              <div className="space-y-1 p-4 pt-0">
                {tools.map((tool) => (
                  <button
                    key={tool.id}
                    onClick={() => setSelectedTool(tool)}
                    className={`w-full text-left p-3 rounded-lg transition-colors ${
                      selectedTool?.id === tool.id
                        ? 'bg-[var(--accent)] text-[var(--text-on-accent)]'
                        : 'bg-[var(--bg-muted)] text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/0.25)]'
                    }`}
                  >
                    <div className="flex items-center justify-between mb-1">
                      <span className="font-medium text-sm flex items-center gap-2">
                        <Package className="w-4 h-4" />
                        {tool.name}
                      </span>
                      {tool.breakingChanges && (
                        <AlertTriangle className="w-4 h-4 text-[rgb(var(--warning-rgb)/0.9)]" />
                      )}
                    </div>
                    <div className="flex items-center gap-2 mt-2">
                      {getVersionBadge(tool.version)}
                      {getStatusBadge(tool.status)}
                    </div>
                    {tool.dependentAgents.length > 0 && (
                      <div className="text-xs text-[var(--text-muted)] mt-1">
                        Used by {tool.dependentAgents.length} agent(s)
                      </div>
                    )}
                  </button>
                ))}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>

        {/* Main Editor Area */}
        <Card className="flex-1 bg-[var(--bg-secondary)] border-[var(--border-color)] overflow-hidden">
          {selectedTool ? (
            <>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <div>
                    <CardTitle className="flex items-center gap-2">
                      {selectedTool.name}
                      {getVersionBadge(selectedTool.version)}
                      {getStatusBadge(selectedTool.status)}
                    </CardTitle>
                    <CardDescription className="mt-1">
                      {selectedTool.description}
                    </CardDescription>
                  </div>
                  {selectedTool.status === 'active' && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleMarkLegacy}
                    >
                      Mark as Legacy
                    </Button>
                  )}
                </div>
              </CardHeader>

              <CardContent>
                <ScrollArea className="h-[calc(100vh-300px)] pr-4">
                  <div className="space-y-6">
                    {/* Breaking Changes Warning */}
                    {selectedTool.breakingChanges && selectedTool.dependentAgents.length > 0 && (
                      <Alert className="bg-[rgb(var(--warning-rgb)/0.2)] border-[rgb(var(--warning-rgb)/0.6)]">
                        <AlertTriangle className="h-4 w-4 text-[var(--warning)]" />
                        <AlertDescription className="text-[rgb(var(--warning-rgb)/0.7)]">
                          <strong>Breaking Change Detected!</strong>
                          <p className="mt-1">
                            This tool's input schema has changed. The following agents need updates:
                          </p>
                          <div className="flex flex-wrap gap-2 mt-2">
                            {selectedTool.dependentAgents.map((agent) => (
                              <Badge key={agent} variant="outline" className="text-[rgb(var(--warning-rgb)/0.7)]">
                                {agent}
                              </Badge>
                            ))}
                          </div>
                        </AlertDescription>
                      </Alert>
                    )}

                    {/* Legacy Status */}
                    {selectedTool.status === 'legacy' && (
                      <Alert className="bg-[rgb(var(--info-rgb)/0.12)] border-[rgb(var(--info-rgb)/0.35)]">
                        <AlertCircle className="h-4 w-4 text-[var(--accent)]" />
                        <AlertDescription className="text-[rgb(var(--info-rgb)/0.85)]">
                          <strong>Legacy Tool:</strong> This tool is marked for deprecation.
                          Consider migrating agents to a newer alternative.
                        </AlertDescription>
                      </Alert>
                    )}

                    {/* Basic Info */}
                    <div className="space-y-4">
                      <div>
                        <Label htmlFor="name">Tool Name</Label>
                        <Input
                          id="name"
                          value={selectedTool.name}
                          onChange={(e) => handleFieldChange('name', e.target.value)}
                          className="bg-[var(--bg-muted)] border-[var(--border-color)]"
                        />
                      </div>

                      <div>
                        <Label htmlFor="description">Description</Label>
                        <Input
                          id="description"
                          value={selectedTool.description}
                          onChange={(e) => handleFieldChange('description', e.target.value)}
                          className="bg-[var(--bg-muted)] border-[var(--border-color)]"
                        />
                      </div>

                      <div>
                        <Label htmlFor="version">Version (Semantic)</Label>
                        <div className="flex items-center gap-2">
                          <Input
                            id="version"
                            value={selectedTool.version}
                            onChange={(e) => handleFieldChange('version', e.target.value)}
                            className="bg-[var(--bg-muted)] border-[var(--border-color)]"
                            placeholder="1.0.0"
                          />
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => {
                              const [major, minor, patch] = selectedTool.version.split('.').map(Number);
                              handleFieldChange('version', `${major}.${minor}.${patch + 1}`);
                            }}
                          >
                            <ArrowUpCircle className="w-4 h-4 mr-1" />
                            Patch
                          </Button>
                        </div>
                        <p className="text-xs text-[var(--text-muted)] mt-1">
                          Auto-increments on save. Use major.minor.patch format.
                        </p>
                      </div>
                    </div>

                    {/* Input Schema */}
                    <div>
                      <Label htmlFor="inputSchema">Input Schema (JSON)</Label>
                      <Textarea
                        id="inputSchema"
                        value={JSON.stringify(selectedTool.inputSchema, null, 2)}
                        onChange={(e) => {
                          try {
                            handleFieldChange('inputSchema', JSON.parse(e.target.value));
                          } catch (err) {
                            // Invalid JSON, ignore
                          }
                        }}
                        className="bg-[var(--bg-muted)] border-[var(--border-color)] min-h-[200px] font-mono text-sm"
                      />
                      <p className="text-xs text-[var(--text-muted)] mt-1">
                        Changes to this schema will trigger a breaking change warning
                      </p>
                    </div>

                    {/* Script */}
                    <div>
                      <Label htmlFor="script">Tool Script</Label>
                      <Textarea
                        id="script"
                        value={selectedTool.script}
                        onChange={(e) => handleFieldChange('script', e.target.value)}
                        className="bg-[var(--bg-muted)] border-[var(--border-color)] min-h-[300px] font-mono text-sm"
                        placeholder="#!/bin/bash\n# Your tool script here..."
                      />
                    </div>

                    {/* Dependent Agents */}
                    {selectedTool.dependentAgents.length > 0 && (
                      <div>
                        <Label>Dependent Agents</Label>
                        <div className="flex flex-wrap gap-2 mt-2">
                          {selectedTool.dependentAgents.map((agent) => (
                            <Badge key={agent} variant="secondary">
                              {agent}
                            </Badge>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </ScrollArea>
              </CardContent>
            </>
          ) : (
            <CardContent className="flex items-center justify-center h-full">
              <p className="text-[var(--text-muted)]">Select a tool to begin editing</p>
            </CardContent>
          )}
        </Card>
      </div>
    </div>
  );
}
