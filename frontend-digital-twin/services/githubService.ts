/**
 * GitHub API Service for Global Sync Dashboard
 * 
 * Fetches repository traffic data (clones and views) to track
 * the expansion of the decentralized AGI network.
 * 
 * Requires a GitHub Personal Access Token with 'Administration: Read' permissions.
 */

export interface GitHubCloneData {
  timestamp: string;
  count: number;
  uniques: number;
}

export interface GitHubViewData {
  timestamp: string;
  count: number;
  uniques: number;
}

export interface GitHubTrafficResponse {
  clones: GitHubCloneData[];
  views: GitHubViewData[];
}

const GITHUB_API_BASE = 'https://api.github.com';
const REPO_OWNER = 'c04ch1337';
const REPO_NAME = 'pagi-agent-repo';

/**
 * Fetch repository clone traffic data
 */
export async function fetchCloneTraffic(
  token: string
): Promise<GitHubCloneData[]> {
  const url = `${GITHUB_API_BASE}/repos/${REPO_OWNER}/${REPO_NAME}/traffic/clones`;
  
  const response = await fetch(url, {
    headers: {
      'Authorization': `token ${token}`,
      'Accept': 'application/vnd.github.v3+json',
    },
  });

  if (!response.ok) {
    if (response.status === 404) {
      throw new Error('Repository not found or access denied. Check your GitHub token permissions.');
    }
    if (response.status === 403) {
      throw new Error('GitHub API rate limit exceeded or insufficient permissions.');
    }
    throw new Error(`GitHub API error: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();
  return data.clones || [];
}

/**
 * Fetch repository view traffic data
 */
export async function fetchViewTraffic(
  token: string
): Promise<GitHubViewData[]> {
  const url = `${GITHUB_API_BASE}/repos/${REPO_OWNER}/${REPO_NAME}/traffic/views`;
  
  const response = await fetch(url, {
    headers: {
      'Authorization': `token ${token}`,
      'Accept': 'application/vnd.github.v3+json',
    },
  });

  if (!response.ok) {
    if (response.status === 404) {
      throw new Error('Repository not found or access denied. Check your GitHub token permissions.');
    }
    if (response.status === 403) {
      throw new Error('GitHub API rate limit exceeded or insufficient permissions.');
    }
    throw new Error(`GitHub API error: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();
  return data.views || [];
}

/**
 * Fetch both clone and view traffic data
 */
export async function fetchTrafficData(
  token: string
): Promise<GitHubTrafficResponse> {
  const [clones, views] = await Promise.all([
    fetchCloneTraffic(token),
    fetchViewTraffic(token),
  ]);

  return { clones, views };
}

/**
 * Calculate network health percentage
 * Based on the ratio of unique clones to total views
 */
export function calculateNetworkHealth(
  clones: GitHubCloneData[],
  views: GitHubViewData[]
): number {
  if (views.length === 0) return 0;

  const totalUniqueClones = clones.reduce((sum, day) => sum + day.uniques, 0);
  const totalUniqueViews = views.reduce((sum, day) => sum + day.uniques, 0);

  if (totalUniqueViews === 0) return 0;

  // Network health = (unique clones / unique views) * 100
  // Higher ratio means more nodes are actively cloning (good)
  const healthRatio = totalUniqueClones / totalUniqueViews;
  return Math.min(100, Math.round(healthRatio * 100));
}
