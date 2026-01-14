import React, { useState, useEffect } from 'react';
import {
  GitCommit,
  RotateCcw,
  User,
  Clock,
  FileText,
  AlertTriangle,
  CheckCircle
} from 'lucide-react';

interface CommitHistory {
  hash: string;
  author: string;
  timestamp: string;
  message: string;
  files: string[];
  isActive: boolean;
}

interface ForgeHistoryProps {
  agentId: string;
}

export default function ForgeHistory({ agentId }: ForgeHistoryProps) {
  const [history, setHistory] = useState<CommitHistory[]>([]);
  const [selectedCommit, setSelectedCommit] = useState<CommitHistory | null>(null);
  const [diff, setDiff] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [reverting, setReverting] = useState(false);

  useEffect(() => {
    fetchHistory();
  }, [agentId]);

  const fetchHistory = async () => {
    setLoading(true);
    try {
      const response = await fetch(`/api/agents/${agentId}/history`);
      const data = await response.json();
      setHistory(data);
      if (data.length > 0) {
        setSelectedCommit(data[0]);
        fetchDiff(data[0].hash);
      }
    } catch (error) {
      console.error('Failed to fetch history:', error);
    } finally {
      setLoading(false);
    }
  };

  const fetchDiff = async (commitHash: string) => {
    try {
      const response = await fetch(`/api/agents/${agentId}/diff/${commitHash}`);
      const data = await response.json();
      setDiff(data.diff);
    } catch (error) {
      console.error('Failed to fetch diff:', error);
    }
  };

  const handleRevert = async (commitHash: string) => {
    if (!confirm('Are you sure you want to revert to this version? This will create a new commit.')) {
      return;
    }

    setReverting(true);
    try {
      const response = await fetch(`/api/agents/${agentId}/revert`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ commitHash }),
      });

      if (response.ok) {
        await fetchHistory();
        // Trigger discovery refresh
        await fetch('/api/agents/discovery-refresh', { method: 'POST' });
      }
    } catch (error) {
      console.error('Failed to revert:', error);
    } finally {
      setReverting(false);
    }
  };

  const handleCommitSelect = (commit: CommitHistory) => {
    setSelectedCommit(commit);
    fetchDiff(commit.hash);
  };

  return (
    <div className="flex gap-4 h-full">
      {/* Commit List */}
      <div className="w-1/3">
        <h3 className="text-sm font-semibold mb-3 text-[var(--text-secondary)]">Version History</h3>
        <ScrollArea className="h-[calc(100%-2rem)]">
          <div className="space-y-2">
            {history.map((commit) => (
              <button
                key={commit.hash}
                onClick={() => handleCommitSelect(commit)}
                className={`w-full text-left p-3 rounded-lg transition-colors ${
                  selectedCommit?.hash === commit.hash
                    ? 'bg-[var(--accent)] text-[var(--text-on-accent)]'
                    : 'bg-[var(--bg-muted)] text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/0.25)]'
                }`}
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <GitCommit className="w-4 h-4" />
                    <span className="font-mono text-xs">{commit.hash.substring(0, 7)}</span>
                  </div>
                  {commit.isActive && (
                    <Badge variant="default" className="text-xs">
                      <CheckCircle className="w-3 h-3 mr-1" />
                      Active
                    </Badge>
                  )}
                </div>
                <p className="text-sm font-medium mb-1">{commit.message}</p>
                <div className="flex items-center gap-3 text-xs text-[var(--text-muted)]">
                  <span className="flex items-center gap-1">
                    <User className="w-3 h-3" />
                    {commit.author}
                  </span>
                  <span className="flex items-center gap-1">
                    <Clock className="w-3 h-3" />
                    {new Date(commit.timestamp).toLocaleDateString()}
                  </span>
                </div>
                <div className="flex items-center gap-1 mt-2">
                  {commit.files.map((file, idx) => (
                    <Badge key={idx} variant="outline" className="text-xs">
                      <FileText className="w-3 h-3 mr-1" />
                      {file.split('/').pop()}
                    </Badge>
                  ))}
                </div>
              </button>
            ))}
          </div>
        </ScrollArea>
      </div>

      {/* Diff Viewer */}
      <div className="flex-1 flex flex-col">
        {selectedCommit ? (
          <>
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-[var(--text-secondary)]">Changes</h3>
              {!selectedCommit.isActive && (
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => handleRevert(selectedCommit.hash)}
                  disabled={reverting}
                >
                  <RotateCcw className="w-4 h-4 mr-2" />
                  {reverting ? 'Reverting...' : 'Revert to This Version'}
                </Button>
              )}
            </div>

            {!selectedCommit.isActive && (
              <Alert className="mb-3 bg-[rgb(var(--warning-rgb)/0.2)] border-[rgb(var(--warning-rgb)/0.6)]">
                <AlertTriangle className="h-4 w-4 text-[var(--warning)]" />
                <AlertDescription className="text-[rgb(var(--warning-rgb)/0.7)]">
                  This is a historical version. Reverting will create a new commit with these changes.
                </AlertDescription>
              </Alert>
            )}

            <ScrollArea className="flex-1 bg-[var(--bg-primary)] rounded-lg p-4">
              <pre className="text-xs font-mono text-[var(--text-secondary)] whitespace-pre-wrap">
                {diff || 'Loading diff...'}
              </pre>
            </ScrollArea>
          </>
        ) : (
          <div className="flex items-center justify-center h-full text-[var(--text-muted)]">
            Select a commit to view changes
          </div>
        )}
      </div>
    </div>
  );
}
