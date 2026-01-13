export enum TwinStatus {
  IDLE = 'IDLE',
  THINKING = 'THINKING',
  EXECUTING = 'EXECUTING',
  OFFLINE = 'OFFLINE'
}

export interface TwinSettings {
  safeMode: boolean;
  toolAccess: string[];
  maxMemory: number;
  tokenLimit: number;
  memoryNamespace: string;
  aiCodeGenerationEnabled: boolean;
  llmProvider: 'gemini' | 'openai' | 'anthropic' | 'llama' | 'deepseek' | 'mistral' | 'grok' | 'openrouter' | 'ollama';
  apiKey?: string;
  temperature: number;
  topP: number;
}

export interface Twin {
  id: string;
  name: string;
  role: string;
  description: string;
  avatar: string;
  status: TwinStatus;
  systemPrompt: string;
  capabilities: string[];
  settings: TwinSettings;
  isOrchestrator?: boolean;
  isTacticalNode: boolean;
}

export interface Message {
  id: string;
  sender: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  twinId?: string;
  thinking?: string;
  attachments?: {
    type: 'image' | 'video' | 'code';
    url?: string;
    content?: string;
  }[];
}

export interface LogEntry {
  id: string;
  timestamp: Date;
  level: 'info' | 'warn' | 'error' | 'plan' | 'tool' | 'memory';
  message: string;
}

export interface Job {
  id: string;
  twinId: string;
  name: string;
  progress: number;
  status: 'pending' | 'active' | 'completed' | 'failed';
  logs?: LogEntry[];
  startTime?: Date;
}

export interface Approval {
  id: string;
  twinId: string;
  action: string;
  description: string;
  timestamp: Date;
  status: 'pending' | 'approved' | 'denied';
}

export interface TelemetryData {
  cpu: number;
  memory: number;
  gpu: number;
  network: number;
  timestamp: string;
}

export type AppView = 'chat' | 'settings' | 'orchestrator' | 'logs' | 'search' | 'memory-explorer' | 'evolution' | 'gallery' | 'system-status';