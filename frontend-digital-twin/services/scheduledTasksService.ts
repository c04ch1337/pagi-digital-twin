/**
 * Scheduled Tasks Service
 * Manages Phoenix Chronos scheduled tasks
 */

const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

export interface ScheduledTask {
  id: string;
  name: string;
  cron_expression: string;
  agent_id?: string;
  task_payload: Record<string, any>;
  status: 'pending' | 'running' | 'completed' | 'failed';
  created_at: string;
  last_run?: string;
  next_run?: string;
}

export interface CreateScheduledTaskRequest {
  name: string;
  cron_expression: string;
  agent_id?: string;
  task_payload: Record<string, any>;
}

export interface ScheduledTaskResponse {
  task: ScheduledTask;
}

export interface ScheduledTasksListResponse {
  tasks: ScheduledTask[];
}

const getOrchestratorUrl = (): string => {
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

async function requestScheduledTasksApi<T>(path: string, init: RequestInit): Promise<T> {
  const orchestratorUrl = getOrchestratorUrl();
  const url = `${orchestratorUrl}${path}`;

  const res = await fetch(url, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init.headers || {}),
    },
  });

  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`${init.method || 'GET'} ${url} failed: ${res.status} ${res.statusText}${body ? ` - ${body}` : ''}`);
  }

  return (await res.json()) as T;
}

export async function listScheduledTasks(): Promise<ScheduledTask[]> {
  const response = await requestScheduledTasksApi<ScheduledTasksListResponse>('/api/scheduled-tasks', {
    method: 'GET',
  });
  return response.tasks;
}

export async function getScheduledTask(taskId: string): Promise<ScheduledTask> {
  const response = await requestScheduledTasksApi<ScheduledTaskResponse>(`/api/scheduled-tasks/${taskId}`, {
    method: 'GET',
  });
  return response.task;
}

export async function createScheduledTask(request: CreateScheduledTaskRequest): Promise<ScheduledTask> {
  const response = await requestScheduledTasksApi<ScheduledTaskResponse>('/api/scheduled-tasks', {
    method: 'POST',
    body: JSON.stringify(request),
  });
  return response.task;
}

export async function deleteScheduledTask(taskId: string): Promise<void> {
  await requestScheduledTasksApi<{ ok: boolean }>(`/api/scheduled-tasks/${taskId}`, {
    method: 'DELETE',
  });
}
