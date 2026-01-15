import { useState, useEffect, useRef } from 'react';
import { getIngestStatus, IngestionStatus } from '../services/knowledgeService';

interface UseIngestStatusOptions {
  pollInterval?: number; // milliseconds, default 2000
  enabled?: boolean; // default true
}

interface UseIngestStatusReturn {
  status: IngestionStatus;
  isLoading: boolean;
  error: Error | null;
}

/**
 * Custom hook for polling ingestion status
 * Polls the /api/knowledge/ingest/status endpoint at regular intervals
 */
export function useIngestStatus(
  options: UseIngestStatusOptions = {}
): UseIngestStatusReturn {
  const { pollInterval = 2000, enabled = true } = options;
  const [status, setStatus] = useState<IngestionStatus>({
    is_active: false,
    files_processed: 0,
    files_failed: 0,
    current_file: null,
    last_error: null,
  });
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const previousStatusRef = useRef<IngestionStatus | null>(null);

  useEffect(() => {
    if (!enabled) return;

    let isMounted = true;
    let pollIntervalId: NodeJS.Timeout | null = null;

    const poll = async () => {
      try {
        const newStatus = await getIngestStatus();
        if (isMounted) {
          setStatus(newStatus);
          setIsLoading(false);
          setError(null);
          previousStatusRef.current = newStatus;
        }
      } catch (err) {
        if (isMounted) {
          setError(err instanceof Error ? err : new Error('Unknown error'));
          setIsLoading(false);
        }
      }
    };

    // Initial poll
    poll();

    // Set up polling interval
    pollIntervalId = setInterval(poll, pollInterval);

    return () => {
      isMounted = false;
      if (pollIntervalId) {
        clearInterval(pollIntervalId);
      }
    };
  }, [pollInterval, enabled]);

  return { status, isLoading, error };
}
