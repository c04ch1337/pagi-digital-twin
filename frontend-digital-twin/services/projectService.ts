/**
 * Service for managing project/application watch folder configuration
 */

const ORCHESTRATOR_URL = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

export interface ConfigureWatchRequest {
  project_id: string;
  project_name: string;
  watch_path: string;
}

export interface ConfigureWatchResponse {
  ok: boolean;
  message: string;
}

/**
 * Configures a watch folder for a project/application
 */
export async function configureProjectWatch(
  projectId: string,
  projectName: string,
  watchPath: string
): Promise<ConfigureWatchResponse> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/projects/configure-watch`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      project_id: projectId,
      project_name: projectName,
      watch_path: watchPath,
    }),
  });

  if (!response.ok) {
    const errorText = await response.text().catch(() => response.statusText);
    throw new Error(`Failed to configure watch path: ${errorText}`);
  }

  return response.json();
}

/**
 * Gets the current watch configuration for all projects
 */
export async function getWatchConfigurations(): Promise<Record<string, string>> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/projects/watch-configs`, {
    method: 'GET',
    headers: { Accept: 'application/json' },
  });

  if (!response.ok) {
    throw new Error(`Failed to get watch configurations: ${response.statusText}`);
  }

  return response.json();
}

export interface FileProcessingEvent {
  project_id: string;
  project_name: string;
  file_path: string;
  file_name: string;
  file_type: string;
  status: 'success' | 'error' | 'skipped';
  error_message?: string;
  memory_id?: string;
  namespace: string;
  timestamp: string;
  file_size: number;
}

export interface FileProcessingStats {
  project_id: string;
  project_name: string;
  total_processed: number;
  successful: number;
  failed: number;
  skipped: number;
  last_processed?: string;
  recent_events: FileProcessingEvent[];
}

/**
 * Gets file processing statistics for all projects or a specific project
 */
export async function getProcessingStats(projectId?: string): Promise<FileProcessingStats[]> {
  const url = new URL(`${ORCHESTRATOR_URL}/api/projects/processing-stats`);
  if (projectId) {
    url.searchParams.set('project_id', projectId);
  }

  const response = await fetch(url.toString(), {
    method: 'GET',
    headers: { Accept: 'application/json' },
  });

  if (!response.ok) {
    throw new Error(`Failed to get processing stats: ${response.statusText}`);
  }

  return response.json();
}
