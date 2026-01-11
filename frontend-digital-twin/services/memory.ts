export interface VectorShard {
  id: string;
  text: string;
  timestamp: Date;
  metadata?: any;
}

export interface MemoryStatus {
  used: number;
  total: number;
  namespace: string;
  load: number;
  shardCount: number;
}

// Global in-memory vault for tactical data
const memoryVault: Record<string, VectorShard[]> = {
  'threat_intel_v24': [
    { id: 's1', text: 'Indicator of Compromise (IOC) detected: C2 beaconing to 185.x.x.x identified in outbound firewall logs.', timestamp: new Date(Date.now() - 3600000) },
    { id: 's2', text: 'Policy Alert: Administrative accounts must use MFA for all lateral RDP sessions.', timestamp: new Date(Date.now() - 7200000) }
  ]
};

/**
 * Commits a new tactical shard to the vector vault.
 */
export const commitToMemory = (namespace: string, text: string): VectorShard => {
  if (!memoryVault[namespace]) {
    memoryVault[namespace] = [];
  }
  
  const newShard: VectorShard = {
    id: `shard-${Math.random().toString(36).substr(2, 9)}`,
    text,
    timestamp: new Date(),
  };
  
  memoryVault[namespace].push(newShard);
  return newShard;
};

/**
 * Retrieves all knowledge shards for a given namespace.
 */
export const getNamespaceShards = (namespace: string): VectorShard[] => {
  return memoryVault[namespace] || [];
};

/**
 * Tactical Memory Service
 * Provides metrics and access to knowledge shards.
 */
export const fetchNamespaceMetrics = (namespace: string): MemoryStatus => {
  const shards = getNamespaceShards(namespace);
  const shardCount = shards.length;
  // Each shard "uses" roughly 0.5MB of vectorized space in our simulation
  const used = Math.min(1024, (shardCount * 1.5) + (Math.random() * 5)); 
  const total = 1024;
  
  return {
    used: Math.round(used),
    total,
    namespace,
    load: Math.round((used / total) * 100),
    shardCount
  };
};
