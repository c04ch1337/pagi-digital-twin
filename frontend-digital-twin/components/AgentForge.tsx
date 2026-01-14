import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { 
  Save, 
  RefreshCw, 
  History, 
  TestTube, 
  AlertTriangle,
  CheckCircle,
  Clock,
  GitBranch,
  User,
  Shield,
  Vote,
  XCircle
} from 'lucide-react';
import ForgeHistory from './ForgeHistory';
import AgentWarRoom from './AgentWarRoom';
import ConsensusOverrideModal from './ConsensusOverrideModal';

interface Agent {
  id: string;
  name: string;
  description: string;
  prompt: string;
  tools: string[];
  status: 'active' | 'draft' | 'deprecated';
  version: string;
  lastModified: string;
  modifiedBy: string;
  // Phoenix Consensus fields
  consensusStatus?: 'approved' | 'pending' | 'rejected' | 'quarantined';
  meshTrustScore?: number; // 0-100
  approvalPercentage?: number; // 0-100
  totalVotes?: number;
  quarantineReason?: string;
  commitHash?: string;
}

interface AgentForgeProps {
  className?: string;
}

export default function AgentForge({ className }: AgentForgeProps) {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<Agent | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [activeTab, setActiveTab] = useState('editor');
  const [castingVote, setCastingVote] = useState(false);
  const [showOverrideModal, setShowOverrideModal] = useState(false);

  // Fetch agents from backend
  useEffect(() => {
    fetchAgents();
  }, []);

  const fetchAgents = async () => {
    setLoading(true);
    try {
      const response = await fetch('/api/agents');
      const data = await response.json();
      setAgents(data);
      if (data.length > 0 && !selectedAgent) {
        setSelectedAgent(data[0]);
      }
    } catch (error) {
      console.error('Failed to fetch agents:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!selectedAgent) return;
    
    setSaving(true);
    try {
      const response = await fetch(`/api/agents/${selectedAgent.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(selectedAgent),
      });
      
      if (response.ok) {
        await fetchAgents();
        // Trigger discovery refresh
        await fetch('/api/agents/discovery-refresh', { method: 'POST' });
      }
    } catch (error) {
      console.error('Failed to save agent:', error);
    } finally {
      setSaving(false);
    }
  };

  const handleFieldChange = (field: keyof Agent, value: any) => {
    if (!selectedAgent) return;
    setSelectedAgent({ ...selectedAgent, [field]: value });
  };

  const handleCastVote = async (approved: boolean) => {
    if (!selectedAgent || !selectedAgent.commitHash) return;
    
    // If pending consensus, open override modal instead of directly voting
    if (selectedAgent.consensusStatus === 'pending') {
      setShowOverrideModal(true);
      return;
    }
    
    setCastingVote(true);
    try {
      const response = await fetch('/api/consensus/vote', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          commitHash: selectedAgent.commitHash,
          approved,
        }),
      });
      
      if (response.ok) {
        await fetchAgents();
      }
    } catch (error) {
      console.error('Failed to cast vote:', error);
    } finally {
      setCastingVote(false);
    }
  };

  const handleStrategicOverride = async (rationale: string) => {
    if (!selectedAgent || !selectedAgent.commitHash) return;
    
    const response = await fetch('/api/consensus/override', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        commit_hash: selectedAgent.commitHash,
        rationale,
      }),
    });
    
    if (!response.ok) {
      const data = await response.json();
      throw new Error(data.message || 'Override failed');
    }
    
    await fetchAgents();
  };

  const getConsensusBadge = (agent: Agent) => {
    if (!agent.consensusStatus) return null;
    
    const status = agent.consensusStatus;
    const trustScore = agent.meshTrustScore ?? 0;
    
    if (status === 'quarantined') {
      return (
        <Badge variant="destructive" className="text-xs">
          <XCircle className="w-3 h-3 mr-1" />
          Quarantined
        </Badge>
      );
    }
    
    if (status === 'approved') {
      return (
        <Badge variant="default" className="text-xs bg-[var(--success)] text-[var(--text-on-accent)]">
          <Shield className="w-3 h-3 mr-1" />
          {trustScore}% Approved
        </Badge>
      );
    }
    
    if (status === 'pending') {
      return (
        <Badge variant="secondary" className="text-xs">
          <Clock className="w-3 h-3 mr-1" />
          Pending ({agent.approvalPercentage ?? 0}%)
        </Badge>
      );
    }
    
    if (status === 'rejected') {
      return (
        <Badge variant="destructive" className="text-xs">
          <AlertTriangle className="w-3 h-3 mr-1" />
          Rejected
        </Badge>
      );
    }
    
    return null;
  };

  return (
    <div className={`flex flex-col h-full ${className}`}>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-2xl font-bold text-[var(--text-on-accent)]">AgentForge</h2>
          <p className="text-sm text-[var(--text-muted)]">IT-Strategic Layer: Build & Evolve Agents</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={fetchAgents}
            disabled={loading}
          >
            <RefreshCw className={`w-4 h-4 mr-2 ${loading ? 'animate-spin' : ''}`} />
            Refresh
          </Button>
          <Button
            size="sm"
            onClick={handleSave}
            disabled={!selectedAgent || saving}
          >
            <Save className="w-4 h-4 mr-2" />
            {saving ? 'Saving...' : 'Save & Deploy'}
          </Button>
        </div>
      </div>

      <div className="flex gap-4 flex-1 overflow-hidden">
        {/* Agent List Sidebar */}
        <Card className="w-64 bg-[var(--bg-secondary)] border-[var(--border-color)]">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">Agents</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <ScrollArea className="h-[calc(100vh-250px)]">
              <div className="space-y-1 p-4 pt-0">
                {agents.map((agent) => (
                    <button
                      key={agent.id}
                      onClick={() => setSelectedAgent(agent)}
                      className={`w-full text-left p-3 rounded-lg transition-colors ${
                        selectedAgent?.id === agent.id
                          ? 'bg-[var(--accent)] text-[var(--text-on-accent)]'
                          : 'bg-[var(--bg-muted)] text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/0.45)]'
                      }`}
                    >
                    <div className="flex items-center justify-between mb-1">
                      <span className="font-medium text-sm">{agent.name}</span>
                      <div className="flex items-center gap-1">
                        {getConsensusBadge(agent)}
                        <Badge
                          variant={
                            agent.status === 'active'
                              ? 'default'
                              : agent.status === 'draft'
                              ? 'secondary'
                              : 'destructive'
                          }
                          className="text-xs"
                        >
                          {agent.status}
                        </Badge>
                      </div>
                    </div>
                    <div className="text-xs text-[var(--text-muted)] flex items-center gap-1">
                      <GitBranch className="w-3 h-3" />
                      v{agent.version}
                    </div>
                  </button>
                ))}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>

        {/* Main Editor Area */}
        <Card className="flex-1 bg-[var(--bg-secondary)] border-[var(--border-color)] overflow-hidden">
          {selectedAgent ? (
            <Tabs value={activeTab} onValueChange={setActiveTab} className="h-full flex flex-col">
              <CardHeader className="pb-3">
                <div className="flex items-center justify-between">
                  <div>
                    <CardTitle>{selectedAgent.name}</CardTitle>
                    <CardDescription className="flex items-center gap-2 mt-1">
                      <User className="w-3 h-3" />
                      Modified by {selectedAgent.modifiedBy}
                      <Clock className="w-3 h-3 ml-2" />
                      {new Date(selectedAgent.lastModified).toLocaleString()}
                    </CardDescription>
                  </div>
                  <TabsList>
                    <TabsTrigger value="editor">Editor</TabsTrigger>
                    <TabsTrigger value="history">
                      <History className="w-4 h-4 mr-2" />
                      History
                    </TabsTrigger>
                    <TabsTrigger value="warroom">
                      <TestTube className="w-4 h-4 mr-2" />
                      War Room
                    </TabsTrigger>
                  </TabsList>
                </div>
              </CardHeader>

              <CardContent className="flex-1 overflow-hidden">
                <TabsContent value="editor" className="h-full mt-0">
                  <ScrollArea className="h-full pr-4">
                    <div className="space-y-6">
                      {/* Basic Info */}
                      <div className="space-y-4">
                        <div>
                          <Label htmlFor="name">Agent Name</Label>
                          <Input
                            id="name"
                            value={selectedAgent.name}
                            onChange={(e) => handleFieldChange('name', e.target.value)}
                            className="bg-[var(--bg-muted)] border-[var(--border-color)]"
                          />
                        </div>

                        <div>
                          <Label htmlFor="description">Description</Label>
                          <Input
                            id="description"
                            value={selectedAgent.description}
                            onChange={(e) => handleFieldChange('description', e.target.value)}
                            className="bg-[var(--bg-muted)] border-[var(--border-color)]"
                          />
                        </div>

                        <div>
                          <Label htmlFor="status">Status</Label>
                          <select
                            id="status"
                            value={selectedAgent.status}
                            onChange={(e) => handleFieldChange('status', e.target.value)}
                            className="w-full p-2 rounded-md bg-[var(--bg-muted)] border border-[var(--border-color)] text-[var(--text-on-accent)]"
                          >
                            <option value="active">Active</option>
                            <option value="draft">Draft</option>
                            <option value="deprecated">Deprecated</option>
                          </select>
                        </div>
                      </div>

                      {/* System Prompt */}
                      <div>
                        <Label htmlFor="prompt">System Prompt</Label>
                        <Textarea
                          id="prompt"
                          value={selectedAgent.prompt}
                          onChange={(e) => handleFieldChange('prompt', e.target.value)}
                          className="bg-[var(--bg-muted)] border-[var(--border-color)] min-h-[300px] font-mono text-sm"
                          placeholder="Enter the agent's system prompt..."
                        />
                        <p className="text-xs text-[var(--text-muted)] mt-1">
                          Define the agent's personality, goals, and constraints
                        </p>
                      </div>

                      {/* Tools */}
                      <div>
                        <Label>Available Tools</Label>
                        <div className="flex flex-wrap gap-2 mt-2">
                          {selectedAgent.tools.map((tool) => (
                            <Badge key={tool} variant="secondary">
                              {tool}
                            </Badge>
                          ))}
                        </div>
                        <Button
                          variant="outline"
                          size="sm"
                          className="mt-2"
                          onClick={() => {
                            // TODO: Open tool selector modal
                          }}
                        >
                          Manage Tools
                        </Button>
                      </div>

                      {/* Phoenix Consensus Status */}
                      {selectedAgent.consensusStatus && (
                        <div className="space-y-2">
                          {selectedAgent.consensusStatus === 'quarantined' && (
                            <Alert className="bg-[rgb(var(--danger-rgb)/0.2)] border-[rgb(var(--danger-rgb)/0.6)]">
                              <XCircle className="h-4 w-4 text-[var(--danger)]" />
                              <AlertDescription className="text-[rgb(var(--danger-rgb)/0.65)]">
                                <strong>Mesh-Wide Quarantine:</strong> This agent has been rejected by the Phoenix mesh.
                                {selectedAgent.quarantineReason && (
                                  <p className="mt-1 text-sm">Reason: {selectedAgent.quarantineReason}</p>
                                )}
                              </AlertDescription>
                            </Alert>
                          )}
                          
                          {selectedAgent.consensusStatus === 'pending' && (
                            <Alert className="bg-[rgb(var(--warning-rgb)/0.2)] border-[rgb(var(--warning-rgb)/0.6)]">
                              <Clock className="h-4 w-4 text-[var(--warning)]" />
                              <AlertDescription className="text-[rgb(var(--warning-rgb)/0.7)]">
                                <strong>Pending Consensus:</strong> Waiting for mesh votes.
                                <div className="mt-2 flex items-center gap-2">
                                  <span className="text-sm">
                                    Approval: {selectedAgent.approvalPercentage ?? 0}% ({selectedAgent.totalVotes ?? 0} votes)
                                  </span>
                                  <div className="flex gap-2 ml-auto">
                                    <Button
                                      size="sm"
                                      variant="outline"
                                      onClick={() => handleCastVote(true)}
                                      disabled={castingVote}
                                      className="text-[rgb(var(--success-rgb)/0.85)] border-[rgb(var(--success-rgb)/0.6)]"
                                    >
                                      <Vote className="w-3 h-3 mr-1" />
                                      Approve
                                    </Button>
                                    <Button
                                      size="sm"
                                      variant="outline"
                                      onClick={() => handleCastVote(false)}
                                      disabled={castingVote}
                                      className="text-[rgb(var(--danger-rgb)/0.8)] border-[rgb(var(--danger-rgb)/0.6)]"
                                    >
                                      <XCircle className="w-3 h-3 mr-1" />
                                      Reject
                                    </Button>
                                  </div>
                                </div>
                              </AlertDescription>
                            </Alert>
                          )}
                          
                          {selectedAgent.consensusStatus === 'approved' && (
                            <Alert className="bg-[rgb(var(--success-rgb)/0.15)] border-[rgb(var(--success-rgb)/0.6)]">
                              <CheckCircle className="h-4 w-4 text-[var(--success)]" />
                              <AlertDescription className="text-[rgb(var(--success-rgb)/0.85)]">
                                <strong>Mesh Trust Score:</strong> {selectedAgent.meshTrustScore ?? 0}% Approved
                                <p className="mt-1 text-sm">
                                  This agent has been verified by {selectedAgent.totalVotes ?? 0} node(s) in the Phoenix mesh.
                                </p>
                              </AlertDescription>
                            </Alert>
                          )}
                        </div>
                      )}

                      {/* Compliance Warnings */}
                      <Alert className="bg-[rgb(var(--warning-rgb)/0.2)] border-[rgb(var(--warning-rgb)/0.6)]">
                        <AlertTriangle className="h-4 w-4 text-[var(--warning)]" />
                        <AlertDescription className="text-[rgb(var(--warning-rgb)/0.7)]">
                          <strong>Ferrellgas Compliance:</strong> Ensure this agent maintains
                          the "Visionary Architect" persona and respects privacy boundaries.
                        </AlertDescription>
                      </Alert>
                    </div>
                  </ScrollArea>
                </TabsContent>

                <TabsContent value="history" className="h-full mt-0">
                  <ForgeHistory agentId={selectedAgent.id} />
                </TabsContent>

                <TabsContent value="warroom" className="h-full mt-0">
                  <AgentWarRoom agent={selectedAgent} />
                </TabsContent>
              </CardContent>
            </Tabs>
          ) : (
            <CardContent className="flex items-center justify-center h-full">
              <p className="text-[var(--text-muted)]">Select an agent to begin editing</p>
            </CardContent>
          )}
        </Card>
      </div>

      {/* Consensus Override Modal */}
      {showOverrideModal && selectedAgent && selectedAgent.commitHash && (
        <ConsensusOverrideModal
          commitHash={selectedAgent.commitHash}
          agentName={selectedAgent.name}
          onClose={() => setShowOverrideModal(false)}
          onOverride={handleStrategicOverride}
        />
      )}
    </div>
  );
}
