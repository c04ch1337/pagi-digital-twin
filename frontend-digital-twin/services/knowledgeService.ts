/**
 * Knowledge Base Service
 * Handles knowledge ingestion status and operations
 */

export interface IngestionStatus {
  is_active: boolean;
  files_processed: number;
  files_failed: number;
  current_file: string | null;
  last_error: string | null;
}

export interface KnowledgeIngestResponse {
  success: boolean;
  message: string;
  file_path: string | null;
}

/**
 * Get current ingestion status
 */
export async function getIngestStatus(): Promise<IngestionStatus> {
  try {
    const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';
    const response = await fetch(`${orchestratorUrl}/api/knowledge/ingest/status`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      throw new Error(`Failed to fetch ingestion status: ${response.statusText}`);
    }

    const data = await response.json();
    return data.status as IngestionStatus;
  } catch (error) {
    console.error('[KnowledgeService] Error fetching ingestion status:', error);
    // Return default status on error
    return {
      is_active: false,
      files_processed: 0,
      files_failed: 0,
      current_file: null,
      last_error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

/**
 * Manually trigger ingestion of a file or all files
 */
export async function triggerIngestion(filePath?: string): Promise<KnowledgeIngestResponse> {
  try {
    const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';
    const response = await fetch(`${orchestratorUrl}/api/knowledge/ingest`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        file_path: filePath || null,
      }),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Failed to trigger ingestion: ${errorText}`);
    }

    const data = await response.json();
    return data as KnowledgeIngestResponse;
  } catch (error) {
    console.error('[KnowledgeService] Error triggering ingestion:', error);
    throw error;
  }
}
