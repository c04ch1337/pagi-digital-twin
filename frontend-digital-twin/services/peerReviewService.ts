/**
 * Peer Review Service
 * Manages peer review requests and responses for the Agent Debate Protocol
 */

const DEFAULT_ORCHESTRATOR_URL = 'http://127.0.0.1:8182';

const getOrchestratorUrl = (): string => {
  return (import.meta.env.VITE_ORCHESTRATOR_URL || DEFAULT_ORCHESTRATOR_URL).trim();
};

export interface PeerReview {
  review_id: string;
  tool_proposal_id: string;
  requesting_agent_id: string;
  requesting_agent_name: string;
  expert_agent_id: string;
  expert_agent_name: string;
  tool_name: string;
  github_url: string;
  requesting_reasoning: string;
  expert_decision?: 'concur' | 'object';
  expert_reasoning?: string;
  alternative_playbook_id?: string;
  status: 'pending' | 'reviewed' | 'consensus_reached';
  consensus?: 'approved' | 'rejected';
  created_at: string;
  reviewed_at?: string;
  consensus_at?: string;
}

export interface RequestPeerReviewRequest {
  tool_proposal_id: string;
  requesting_agent_id: string;
  requesting_agent_name: string;
  tool_name: string;
  github_url: string;
  reasoning: string;
}

export interface RequestPeerReviewResponse {
  review_id: string;
  expert_agent_id: string;
  expert_agent_name: string;
  message: string;
}

export interface SubmitPeerReviewRequest {
  review_id: string;
  expert_agent_id: string;
  decision: 'concur' | 'object';
  reasoning: string;
  alternative_playbook_id?: string;
}

export interface PeerReviewsResponse {
  ok: boolean;
  reviews: PeerReview[];
  count: number;
}

async function requestPeerReviewApi<T>(
  path: string,
  init: RequestInit
): Promise<T> {
  const orchestratorUrl = getOrchestratorUrl();
  const url = `${orchestratorUrl}${path}`;

  try {
    const res = await fetch(url, init);
    if (!res.ok) {
      const body = await res.text().catch(() => '');
      throw new Error(`${init.method || 'GET'} ${url} failed: ${res.status} ${res.statusText}${body ? ` - ${body}` : ''}`);
    }
    return (await res.json()) as T;
  } catch (e) {
    const err = e instanceof Error ? e : new Error(String(e));
    throw err;
  }
}

/**
 * Request a peer review for a tool proposal
 */
export async function requestPeerReview(
  request: RequestPeerReviewRequest
): Promise<RequestPeerReviewResponse> {
  return requestPeerReviewApi<RequestPeerReviewResponse>(
    '/api/peer-reviews',
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify(request),
    }
  );
}

/**
 * Submit a peer review response
 */
export async function submitPeerReview(
  request: SubmitPeerReviewRequest
): Promise<{ success: boolean; message: string; consensus?: string }> {
  return requestPeerReviewApi<{ success: boolean; message: string; consensus?: string }>(
    '/api/peer-reviews/submit',
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify(request),
    }
  );
}

/**
 * Get all peer reviews for a tool proposal
 */
export async function getPeerReviewsForProposal(proposalId: string): Promise<PeerReviewsResponse> {
  return requestPeerReviewApi<PeerReviewsResponse>(
    `/api/peer-reviews/proposal/${proposalId}`,
    {
      method: 'GET',
      headers: {
        Accept: 'application/json',
      },
    }
  );
}

/**
 * Get all peer reviews
 */
export async function getAllPeerReviews(): Promise<PeerReviewsResponse> {
  return requestPeerReviewApi<PeerReviewsResponse>(
    '/api/peer-reviews',
    {
      method: 'GET',
      headers: {
        Accept: 'application/json',
      },
    }
  );
}
