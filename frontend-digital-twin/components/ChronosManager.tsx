import React, { useState, useEffect } from 'react';
import {
  listScheduledTasks,
  createScheduledTask,
  deleteScheduledTask,
  ScheduledTask,
  CreateScheduledTaskRequest,
} from '../services/scheduledTasksService';
import { searchAgents, AgentSearchResult } from '../services/agentSearchService';

interface ChronosManagerProps {
  className?: string;
}

type CronPreset = 'hourly' | 'daily' | 'weekly' | 'monthly' | 'custom';

const CRON_PRESETS: Record<CronPreset, string> = {
  hourly: '0 * * * *', // Every hour at minute 0
  daily: '0 9 * * *', // Daily at 9:00 AM
  weekly: '0 9 * * 1', // Every Monday at 9:00 AM
  monthly: '0 9 1 * *', // First day of month at 9:00 AM
  custom: '',
};

const ChronosManager: React.FC<ChronosManagerProps> = ({ className }) => {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState<boolean>(false);
  const [selectedPreset, setSelectedPreset] = useState<CronPreset>('daily');
  const [cronExpression, setCronExpression] = useState<string>('0 9 * * *');
  const [taskName, setTaskName] = useState<string>('');
  const [taskPayload, setTaskPayload] = useState<string>('{\n  "command": "scan_directory",\n  "path": "/Applications",\n  "goal": "Summarize new telemetry or configuration changes."\n}');
  const [agentSearchQuery, setAgentSearchQuery] = useState<string>('');
  const [agentSearchResults, setAgentSearchResults] = useState<AgentSearchResult[]>([]);
  const [selectedAgentId, setSelectedAgentId] = useState<string | undefined>(undefined);
  const [isSearchingAgents, setIsSearchingAgents] = useState<boolean>(false);

  const fetchTasks = async () => {
    try {
      setLoading(true);
      setError(null);
      const fetchedTasks = await listScheduledTasks();
      setTasks(fetchedTasks);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to fetch scheduled tasks';
      setError(msg);
      console.error('[ChronosManager] Failed to fetch tasks:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchTasks();
    // Refresh every 30 seconds
    const interval = setInterval(fetchTasks, 30000);
    return () => clearInterval(interval);
  }, []);

  // Debounced agent search
  useEffect(() => {
    if (!agentSearchQuery.trim()) {
      setAgentSearchResults([]);
      return;
    }

    const timeoutId = setTimeout(async () => {
      setIsSearchingAgents(true);
      try {
        const results = await searchAgents(agentSearchQuery, 5);
        setAgentSearchResults(results);
      } catch (err) {
        console.error('[ChronosManager] Agent search failed:', err);
        setAgentSearchResults([]);
      } finally {
        setIsSearchingAgents(false);
      }
    }, 500);

    return () => clearTimeout(timeoutId);
  }, [agentSearchQuery]);

  const handlePresetChange = (preset: CronPreset) => {
    setSelectedPreset(preset);
    if (preset !== 'custom' && CRON_PRESETS[preset]) {
      setCronExpression(CRON_PRESETS[preset]);
    }
  };

  const handleCreateTask = async () => {
    if (!taskName.trim() || !cronExpression.trim()) {
      alert('Please provide a task name and cron expression');
      return;
    }

    let payload: Record<string, any>;
    try {
      payload = JSON.parse(taskPayload);
    } catch (err) {
      alert('Invalid JSON in task payload');
      return;
    }

    try {
      const request: CreateScheduledTaskRequest = {
        name: taskName,
        cron_expression: cronExpression,
        agent_id: selectedAgentId,
        task_payload: payload,
      };

      await createScheduledTask(request);
      setShowCreateModal(false);
      setTaskName('');
      setCronExpression('0 9 * * *');
      setTaskPayload('{\n  "command": "scan_directory",\n  "path": "/Applications",\n  "goal": "Summarize new telemetry or configuration changes."\n}');
      setSelectedAgentId(undefined);
      setAgentSearchQuery('');
      setAgentSearchResults([]);
      await fetchTasks();
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to create task';
      alert(`Failed to create scheduled task: ${msg}`);
      console.error('[ChronosManager] Failed to create task:', err);
    }
  };

  const handleDeleteTask = async (taskId: string) => {
    if (!confirm('Are you sure you want to delete this scheduled task?')) {
      return;
    }

    try {
      await deleteScheduledTask(taskId);
      await fetchTasks();
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to delete task';
      alert(`Failed to delete scheduled task: ${msg}`);
      console.error('[ChronosManager] Failed to delete task:', err);
    }
  };

  const formatCronExpression = (cron: string): string => {
    // Simple formatting - could be enhanced with a proper cron parser
    const parts = cron.split(' ');
    if (parts.length !== 5) return cron;

    const [minute, hour, day, month, weekday] = parts;
    let description = '';

    if (minute === '0' && hour === '*') {
      description = 'Every hour';
    } else if (minute === '0' && hour !== '*' && day === '*' && month === '*' && weekday === '*') {
      description = `Daily at ${hour.padStart(2, '0')}:00`;
    } else if (minute === '0' && hour !== '*' && day === '*' && month === '*' && weekday !== '*') {
      const days = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
      description = `Every ${days[parseInt(weekday) % 7]} at ${hour.padStart(2, '0')}:00`;
    } else {
      description = cron;
    }

    return description;
  };

  const formatDate = (dateStr?: string): string => {
    if (!dateStr) return 'Never';
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  return (
    <div className={`flex flex-col h-full ${className || ''}`}>
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-sm font-bold uppercase tracking-widest text-[var(--text-primary)]">Phoenix Chronos</h2>
          <p className="text-xs text-[var(--text-secondary)] mt-1">Scheduled Task Management</p>
        </div>
        <button
          onClick={() => setShowCreateModal(true)}
          className="px-4 py-2 text-xs font-bold uppercase tracking-widest bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] hover:bg-[rgb(var(--bg-steel-rgb)/0.9)] transition-all"
        >
          <span className="material-symbols-outlined text-sm align-middle mr-1">add</span>
          New Task
        </button>
      </div>

      {/* Tasks List */}
      {loading ? (
        <div className="flex items-center justify-center py-8 text-[var(--text-secondary)]">
          <span className="material-symbols-outlined animate-spin mr-2">sync</span>
          Loading tasks...
        </div>
      ) : error ? (
        <div className="bg-[rgb(var(--error-rgb)/0.1)] border border-[var(--error)] rounded-lg p-4 text-[var(--error)] text-sm">
          Error: {error}
        </div>
      ) : tasks.length === 0 ? (
        <div className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-8 text-center text-[var(--text-secondary)]">
          <span className="material-symbols-outlined text-4xl mb-2 opacity-50">schedule</span>
          <p className="text-sm">No scheduled tasks yet</p>
          <p className="text-xs mt-1 opacity-70">Create your first proactive task to get started</p>
        </div>
      ) : (
        <div className="space-y-3 flex-1 overflow-y-auto">
          {tasks.map((task) => (
            <div
              key={task.id}
              className="bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg p-4"
            >
              <div className="flex items-start justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-2">
                    <h3 className="text-sm font-bold text-[var(--text-primary)] truncate">{task.name}</h3>
                    <span
                      className={`px-2 py-0.5 text-[10px] font-bold uppercase rounded ${
                        task.status === 'pending'
                          ? 'bg-[rgb(var(--bg-steel-rgb)/0.3)] text-[var(--bg-steel)]'
                          : task.status === 'running'
                          ? 'bg-[rgb(var(--success-rgb)/0.2)] text-[var(--success)]'
                          : task.status === 'completed'
                          ? 'bg-[rgb(var(--success-rgb)/0.2)] text-[var(--success)]'
                          : 'bg-[rgb(var(--error-rgb)/0.2)] text-[var(--error)]'
                      }`}
                    >
                      {task.status}
                    </span>
                  </div>
                  <div className="space-y-1 text-xs text-[var(--text-secondary)]">
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-sm">schedule</span>
                      <span className="font-mono">{task.cron_expression}</span>
                      <span className="opacity-70">({formatCronExpression(task.cron_expression)})</span>
                    </div>
                    {task.agent_id && (
                      <div className="flex items-center gap-2">
                        <span className="material-symbols-outlined text-sm">smart_toy</span>
                        <span>Agent: {task.agent_id}</span>
                      </div>
                    )}
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-sm">update</span>
                      <span>Last run: {formatDate(task.last_run)}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-sm">next_plan</span>
                      <span>Next run: {formatDate(task.next_run)}</span>
                    </div>
                  </div>
                </div>
                <button
                  onClick={() => handleDeleteTask(task.id)}
                  className="px-3 py-1.5 text-xs font-bold uppercase tracking-widest bg-[rgb(var(--error-rgb)/0.1)] text-[var(--error)] rounded border border-[rgb(var(--error-rgb)/0.3)] hover:bg-[rgb(var(--error-rgb)/0.2)] transition-all"
                  title="Delete task"
                >
                  <span className="material-symbols-outlined text-sm">delete</span>
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Create Task Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-[var(--bg-primary)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-xl p-6 w-full max-w-2xl max-h-[90vh] overflow-y-auto">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-sm font-bold uppercase tracking-widest text-[var(--text-primary)]">Create Scheduled Task</h3>
              <button
                onClick={() => setShowCreateModal(false)}
                className="text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
              >
                <span className="material-symbols-outlined">close</span>
              </button>
            </div>

            <div className="space-y-4">
              {/* Task Name */}
              <div>
                <label className="block text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
                  Task Name
                </label>
                <input
                  type="text"
                  value={taskName}
                  onChange={(e) => setTaskName(e.target.value)}
                  placeholder="Morning File Audit"
                  className="w-full px-3 py-2 bg-[var(--bg-muted)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm text-[var(--text-primary)] focus:outline-none focus:border-[var(--bg-steel)]"
                />
              </div>

              {/* Cron Presets */}
              <div>
                <label className="block text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
                  Schedule Preset
                </label>
                <div className="grid grid-cols-4 gap-2">
                  {(['hourly', 'daily', 'weekly', 'monthly'] as CronPreset[]).map((preset) => (
                    <button
                      key={preset}
                      onClick={() => handlePresetChange(preset)}
                      className={`px-3 py-2 text-xs font-bold uppercase tracking-widest rounded-lg border transition-all ${
                        selectedPreset === preset
                          ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)] border-[rgb(var(--bg-steel-rgb)/0.3)]'
                          : 'bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--text-secondary)] border-[rgb(var(--bg-steel-rgb)/0.3)] hover:bg-[var(--bg-muted)]'
                      }`}
                    >
                      {preset}
                    </button>
                  ))}
                </div>
              </div>

              {/* Cron Expression */}
              <div>
                <label className="block text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
                  Cron Expression
                </label>
                <input
                  type="text"
                  value={cronExpression}
                  onChange={(e) => {
                    setCronExpression(e.target.value);
                    setSelectedPreset('custom');
                  }}
                  placeholder="0 9 * * *"
                  className="w-full px-3 py-2 bg-[var(--bg-muted)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm font-mono text-[var(--text-primary)] focus:outline-none focus:border-[var(--bg-steel)]"
                />
                <p className="text-xs text-[var(--text-secondary)] mt-1 opacity-70">
                  Format: minute hour day month weekday (e.g., "0 9 * * *" = Daily at 9:00 AM)
                </p>
              </div>

              {/* Agent Search */}
              <div>
                <label className="block text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
                  Agent Matchmaker (Optional)
                </label>
                <input
                  type="text"
                  value={agentSearchQuery}
                  onChange={(e) => setAgentSearchQuery(e.target.value)}
                  placeholder="Search for best matching agent..."
                  className="w-full px-3 py-2 bg-[var(--bg-muted)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm text-[var(--text-primary)] focus:outline-none focus:border-[var(--bg-steel)]"
                />
                {isSearchingAgents && (
                  <div className="mt-2 text-xs text-[var(--text-secondary)] flex items-center gap-2">
                    <span className="material-symbols-outlined animate-spin text-sm">sync</span>
                    Searching agents...
                  </div>
                )}
                {agentSearchResults.length > 0 && (
                  <div className="mt-2 space-y-2">
                    {agentSearchResults.map((result, idx) => (
                      <div
                        key={result.agent_id}
                        onClick={() => {
                          setSelectedAgentId(result.agent_id);
                          setAgentSearchQuery(result.agent_name);
                          setAgentSearchResults([]);
                        }}
                        className={`p-3 rounded-lg border cursor-pointer transition-all ${
                          selectedAgentId === result.agent_id
                            ? 'bg-[rgb(var(--bg-steel-rgb)/0.2)] border-[var(--bg-steel)]'
                            : idx === 0
                            ? 'bg-[rgb(var(--success-rgb)/0.1)] border-[rgb(var(--success-rgb)/0.3)] hover:bg-[rgb(var(--success-rgb)/0.15)]'
                            : 'bg-[rgb(var(--surface-rgb)/0.4)] border-[rgb(var(--bg-steel-rgb)/0.3)] hover:bg-[var(--bg-muted)]'
                        }`}
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <span className="text-xs font-bold text-[var(--text-primary)] truncate">
                                {result.agent_name}
                              </span>
                              {idx === 0 && (
                                <span className="px-1.5 py-0.5 text-[9px] font-bold uppercase bg-[var(--success)] text-[var(--text-on-accent)] rounded">
                                  Best Match
                                </span>
                              )}
                              <span className="text-[10px] text-[var(--text-secondary)]">
                                {(result.score * 100).toFixed(0)}% match
                              </span>
                            </div>
                            <p className="text-xs text-[var(--text-secondary)] mt-1 truncate">{result.mission}</p>
                            <span className="text-[10px] text-[var(--text-secondary)] opacity-70">
                              Status: {result.status}
                            </span>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
                {selectedAgentId && (
                  <div className="mt-2 p-2 bg-[rgb(var(--bg-steel-rgb)/0.1)] border border-[var(--bg-steel)] rounded-lg">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-[var(--text-primary)]">Selected: {agentSearchQuery}</span>
                      <button
                        onClick={() => {
                          setSelectedAgentId(undefined);
                          setAgentSearchQuery('');
                        }}
                        className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      >
                        Clear
                      </button>
                    </div>
                  </div>
                )}
              </div>

              {/* Task Payload */}
              <div>
                <label className="block text-xs font-bold uppercase tracking-widest text-[var(--text-secondary)] mb-2">
                  Task Payload (JSON)
                </label>
                <textarea
                  value={taskPayload}
                  onChange={(e) => setTaskPayload(e.target.value)}
                  rows={8}
                  className="w-full px-3 py-2 bg-[var(--bg-muted)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-sm font-mono text-[var(--text-primary)] focus:outline-none focus:border-[var(--bg-steel)]"
                />
              </div>

              {/* Actions */}
              <div className="flex items-center justify-end gap-3 pt-4 border-t border-[rgb(var(--bg-steel-rgb)/0.3)]">
                <button
                  onClick={() => setShowCreateModal(false)}
                  className="px-4 py-2 text-xs font-bold uppercase tracking-widest bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--text-secondary)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] hover:bg-[var(--bg-muted)] transition-all"
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreateTask}
                  className="px-4 py-2 text-xs font-bold uppercase tracking-widest bg-[var(--bg-steel)] text-[var(--text-on-accent)] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] hover:bg-[rgb(var(--bg-steel-rgb)/0.9)] transition-all"
                >
                  Create Task
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ChronosManager;
