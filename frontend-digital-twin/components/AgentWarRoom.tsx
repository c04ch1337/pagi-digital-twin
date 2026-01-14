import React, { useState } from 'react';
import { Button } from './ui/button';
import { Textarea } from './ui/textarea';
import { ScrollArea } from './ui/scroll-area';
import { Badge } from './ui/badge';
import { Alert, AlertDescription } from './ui/alert';
import {
  Play,
  Square,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Clock,
  Zap,
  Shield,
  MessageSquare,
  RotateCcw,
  TrendingUp
} from 'lucide-react';

interface Agent {
  id: string;
  name: string;
  prompt: string;
  tools: string[];
}

interface TraceStep {
  type: 'thought' | 'action' | 'observation';
  content: string;
  timestamp: string;
  tool?: string;
}

interface ComplianceResult {
  privacy: { passed: boolean; details: string };
  efficiency: { passed: boolean; details: string };
  tone: { passed: boolean; details: string };
}

interface AgentWarRoomProps {
  agent: Agent;
}

export default function AgentWarRoom({ agent }: AgentWarRoomProps) {
  const [mission, setMission] = useState('');
  const [running, setRunning] = useState(false);
  const [trace, setTrace] = useState<TraceStep[]>([]);
  const [compliance, setCompliance] = useState<ComplianceResult | null>(null);
  const [complianceEnabled, setComplianceEnabled] = useState(true);
  const [rollbackInfo, setRollbackInfo] = useState<{ commit_hash: string; message: string } | null>(null);
  const [complianceScore, setComplianceScore] = useState<number | null>(null);

  const handleRunMission = async () => {
    if (!mission.trim()) return;

    setRunning(true);
    setTrace([]);
    setCompliance(null);

    try {
      const response = await fetch(`/api/agents/${agent.id}/test`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          mission,
          enableCompliance: complianceEnabled,
        }),
      });

      const reader = response.body?.getReader();
      const decoder = new TextDecoder();

      if (reader) {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const chunk = decoder.decode(value);
          const lines = chunk.split('\n');

          for (const line of lines) {
            if (line.startsWith('data: ')) {
              const data = JSON.parse(line.substring(6));
              
              if (data.type === 'trace') {
                setTrace((prev) => [...prev, data.step]);
              } else if (data.type === 'compliance') {
                setCompliance(data.result);
                // Calculate compliance score (0-100)
                const checks = [
                  data.result.privacy.passed,
                  data.result.efficiency.passed,
                  data.result.tone.passed,
                ];
                const passed = checks.filter(Boolean).length;
                const score = (passed / checks.length) * 100;
                setComplianceScore(score);
              } else if (data.type === 'rollback') {
                setRollbackInfo({
                  commit_hash: data.commit_hash,
                  message: data.message,
                });
              }
            }
          }
        }
      }
    } catch (error) {
      console.error('Failed to run mission:', error);
    } finally {
      setRunning(false);
    }
  };

  const handleStop = () => {
    setRunning(false);
    // TODO: Send abort signal to backend
  };

  const getStepIcon = (type: string) => {
      switch (type) {
      case 'thought':
        return <MessageSquare className="w-4 h-4 text-[rgb(var(--info-rgb)/0.9)]" />;
      case 'action':
        return <Zap className="w-4 h-4 text-[rgb(var(--warning-rgb)/0.85)]" />;
      case 'observation':
        return <CheckCircle className="w-4 h-4 text-[rgb(var(--success-rgb)/0.85)]" />;
      default:
        return <Clock className="w-4 h-4 text-[var(--text-muted)]" />;
    }
  };

  const getComplianceIcon = (passed: boolean) => {
    return passed ? (
      <CheckCircle className="w-5 h-5 text-[var(--success)]" />
    ) : (
      <XCircle className="w-5 h-5 text-[rgb(var(--danger-rgb)/0.9)]" />
    );
  };

  return (
    <div className="flex flex-col h-full gap-4">
      {/* Mission Input */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-[var(--text-secondary)]">Test Mission</h3>
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
              <input
                type="checkbox"
                checked={complianceEnabled}
                onChange={(e) => setComplianceEnabled(e.target.checked)}
                className="rounded"
              />
              <Shield className="w-4 h-4" />
              Compliance Check
            </label>
          </div>
        </div>
        <Textarea
          value={mission}
          onChange={(e) => setMission(e.target.value)}
          placeholder="e.g., 'Scan the network and summarize findings'"
          className="bg-[var(--bg-muted)] border-[var(--border-color)] min-h-[100px]"
          disabled={running}
        />
        <div className="flex gap-2">
          {!running ? (
            <Button onClick={handleRunMission} disabled={!mission.trim()}>
              <Play className="w-4 h-4 mr-2" />
              Run Mission
            </Button>
          ) : (
            <Button variant="destructive" onClick={handleStop}>
              <Square className="w-4 h-4 mr-2" />
              Stop
            </Button>
          )}
        </div>
      </div>

      {/* Trace Timeline */}
      {trace.length > 0 && (
        <div className="flex-1 overflow-hidden">
          <h3 className="text-sm font-semibold mb-3 text-[var(--text-secondary)]">Execution Trace</h3>
          <ScrollArea className="h-[calc(100%-2rem)] bg-[var(--bg-primary)] rounded-lg p-4">
            <div className="space-y-4">
              {trace.map((step, idx) => (
                <div key={idx} className="flex gap-3">
                  <div className="flex flex-col items-center">
                    <div className="p-2 rounded-full bg-[var(--bg-muted)]">
                      {getStepIcon(step.type)}
                    </div>
                    {idx < trace.length - 1 && (
                      <div className="w-0.5 h-full bg-[rgb(var(--bg-muted-rgb)/0.9)] mt-2" />
                    )}
                  </div>
                  <div className="flex-1 pb-4">
                    <div className="flex items-center gap-2 mb-1">
                      <Badge variant="outline" className="text-xs">
                        {step.type}
                      </Badge>
                      {step.tool && (
                        <Badge variant="secondary" className="text-xs">
                          {step.tool}
                        </Badge>
                      )}
                      <span className="text-xs text-[var(--text-muted)]">
                        {new Date(step.timestamp).toLocaleTimeString()}
                      </span>
                    </div>
                    <p className="text-sm text-[var(--text-secondary)] whitespace-pre-wrap">
                      {step.content}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </ScrollArea>
        </div>
      )}

      {/* Auto-Rollback Notification */}
      {rollbackInfo && (
        <Alert className="bg-[rgb(var(--warning-rgb)/0.2)] border-[rgb(var(--warning-rgb)/0.6)]">
          <RotateCcw className="h-4 w-4 text-[var(--warning)]" />
          <AlertDescription className="text-[rgb(var(--warning-rgb)/0.7)]">
            <strong>Auto-Rollback Triggered:</strong> {rollbackInfo.message}
            <br />
            <span className="text-xs text-[rgb(var(--warning-rgb)/0.85)]">Reverted to commit: {rollbackInfo.commit_hash.substring(0, 7)}</span>
          </AlertDescription>
        </Alert>
      )}

      {/* Compliance Results */}
      {compliance && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-[var(--text-secondary)]">Compliance Grading</h3>
            {complianceScore !== null && (
              <div className="flex items-center gap-2">
                <TrendingUp className="w-4 h-4 text-[var(--text-muted)]" />
                <span className={`text-sm font-bold ${
                  complianceScore >= 70 ? 'text-[var(--success)]' : 
                  complianceScore >= 50 ? 'text-[rgb(var(--warning-rgb)/0.9)]' : 
                  'text-[rgb(var(--danger-rgb)/0.9)]'
                }`}>
                  {complianceScore.toFixed(0)}%
                </span>
              </div>
            )}
          </div>
          <div className="grid grid-cols-3 gap-3">
            {/* Privacy */}
            <div className="bg-[var(--bg-muted)] rounded-lg p-4">
              <div className="flex items-center gap-2 mb-2">
                {getComplianceIcon(compliance.privacy.passed)}
                <span className="font-medium text-sm">Privacy</span>
              </div>
              <p className="text-xs text-[var(--text-muted)]">{compliance.privacy.details}</p>
            </div>

            {/* Efficiency */}
            <div className="bg-[var(--bg-muted)] rounded-lg p-4">
              <div className="flex items-center gap-2 mb-2">
                {getComplianceIcon(compliance.efficiency.passed)}
                <span className="font-medium text-sm">Efficiency</span>
              </div>
              <p className="text-xs text-[var(--text-muted)]">{compliance.efficiency.details}</p>
            </div>

            {/* Tone */}
            <div className="bg-[var(--bg-muted)] rounded-lg p-4">
              <div className="flex items-center gap-2 mb-2">
                {getComplianceIcon(compliance.tone.passed)}
                <span className="font-medium text-sm">Tone</span>
              </div>
              <p className="text-xs text-[var(--text-muted)]">{compliance.tone.details}</p>
            </div>
          </div>

          {(!compliance.privacy.passed || !compliance.efficiency.passed || !compliance.tone.passed) && (
            <Alert className="bg-[rgb(var(--danger-rgb)/0.2)] border-[rgb(var(--danger-rgb)/0.6)]">
              <AlertTriangle className="h-4 w-4 text-[var(--danger)]" />
              <AlertDescription className="text-[rgb(var(--danger-rgb)/0.65)]">
                <strong>Compliance Issues Detected:</strong> Review the agent's behavior before deploying to production.
              </AlertDescription>
            </Alert>
          )}
        </div>
      )}
    </div>
  );
}
