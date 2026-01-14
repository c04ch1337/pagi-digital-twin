import React from 'react';
import { Twin, TwinStatus } from './types';

export const AVAILABLE_TOOLS = [
  { id: 'file_write', name: 'file_write', label: 'File System Write', icon: 'description', desc: 'Allows the node to write logs, artifacts, and tactical patches to disk.' },
  { id: 'command_exec', name: 'command_exec', label: 'Shell Execution', icon: 'terminal', desc: 'HIGH RISK: Grants permission to execute system-level shell commands.' },
  { id: 'vector_query', name: 'vector_query', label: 'Vector Database', icon: 'database', desc: 'Enable querying of decentralized vector knowledge clusters.' },
  { id: 'network_scan', name: 'network_scan', label: 'Network Recon', icon: 'radar', desc: 'Permission to perform active probes of specified network segments.' },
  { id: 'process_kill', name: 'process_kill', label: 'Process Terminate', icon: 'dangerous', desc: 'Allows force-stopping of suspicious system processes.' }
];

export const INITIAL_TWINS: Twin[] = [
  {
    id: 'twin-aegis',
    name: 'Phoenix',
    role: 'SOAR Orchestrator',
    description: 'Central Incident Response and Security Orchestration.',
    avatar: 'https://images.unsplash.com/photo-1550751827-4bd374c3f58b?auto=format&fit=crop&q=80&w=200',
    status: TwinStatus.IDLE,
    isOrchestrator: true,
    isTacticalNode: true,
    systemPrompt: '# INCIDENT RESPONSE MANDATE\nYou are Phoenix (Ferrellgas Blue Flame), the central SOAR (Security Orchestration, Automation, and Response) brain. Your objective is to coordinate defensive measures, manage the Blue Team lifecycle, and ensure zero-trust policy adherence.\n\n# PROTOCOLS\n1. Analyze alerts for true positives before escalating.\n2. Delegate malware analysis to Sentinel and log forensics to Trace.\n3. Always prioritize system availability and data integrity.',
    capabilities: ['Incident Response', 'Threat Intel Integration', 'SIEM Coordination'],
    settings: {
      safeMode: true,
      toolAccess: ['file_write', 'vector_query'],
      maxMemory: 8,
      tokenLimit: 64,
      memoryNamespace: 'threat_intel',
      aiCodeGenerationEnabled: false,
      llmProvider: 'openrouter',
      temperature: 0.7,
      topP: 0.9
    }
  },
  {
    id: 'twin-sentinel',
    name: 'Sentinel Script',
    role: 'Malware Analyst',
    description: 'Deep file inspection, reverse engineering, and patching.',
    avatar: 'https://images.unsplash.com/photo-1563986768609-322da13575f3?auto=format&fit=crop&q=80&w=200',
    status: TwinStatus.IDLE,
    isTacticalNode: true,
    systemPrompt: '# MALWARE ANALYSIS PROTOCOL\nYou are Sentinel Script. You specialize in analyzing suspicious binaries, PowerShell obfuscation, and automated vulnerability patching.\n\n# CONSTRAINTS\n- Operate only within the isolated "sandbox_v4" namespace.\n- Do not execute commands without Phoenix verification.\n- Report all indicators of compromise (IOCs) immediately.',
    capabilities: ['PowerShell Decoding', 'Binary Reversing', 'Vulnerability Patching'],
    settings: {
      safeMode: false,
      toolAccess: ['file_write', 'command_exec', 'vector_query'],
      maxMemory: 4,
      tokenLimit: 48,
      memoryNamespace: 'sandbox_quarantine',
      aiCodeGenerationEnabled: false,
      llmProvider: 'openrouter',
      temperature: 0.4,
      topP: 0.85
    }
  },
  {
    id: 'twin-trace',
    name: 'Trace Insight',
    role: 'Forensic Investigator',
    description: 'Network traffic analysis and audit log reconstruction.',
    avatar: 'https://images.unsplash.com/photo-1558494949-ef010cbdcc31?auto=format&fit=crop&q=80&w=200',
    status: TwinStatus.IDLE,
    isTacticalNode: true,
    systemPrompt: '# FORENSIC INVESTIGATION FRAMEWORK\nYou are Trace Insight. You find footprints in network traffic and system logs. You specialize in reconstructing attack timelines (Kill Chain analysis).\n\n# OPERATIONAL GOALS\n1. Identify lateral movement patterns.\n2. Correlate VPN logs with endpoint activity.\n3. Visualize traffic anomalies for the human operator.',
    capabilities: ['PCAP Analysis', 'Log Correlation', 'Anomaly Detection'],
    settings: {
      safeMode: true,
      toolAccess: ['vector_query'],
      maxMemory: 12,
      tokenLimit: 128,
      memoryNamespace: 'network_traffic_logs',
      aiCodeGenerationEnabled: false,
      llmProvider: 'openrouter',
      temperature: 0.2,
      topP: 0.95
    }
  }
];

export const ICONS = {
  Cpu: () => (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M15 2v2"/><path d="M15 20v2"/><path d="M2 15h2"/><path d="M2 9h2"/><path d="M20 15h2"/><path d="M20 9h2"/><path d="M9 2v2"/><path d="M9 20v2"/></svg>
  ),
  Brain: () => (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M9.5 2A5 5 0 0 1 12 10a5 5 0 0 1 2.5-8"/><path d="M2.1 12.9A5 5 0 0 1 10 9.5a5 5 0 0 1-8 2.5"/><path d="M21.9 12.9a5 5 0 0 0-8-2.5 5 5 0 0 0 7.9 2.5"/><path d="M12 21.4a5 5 0 0 1-2.5-8.5 5 5 0 0 1 5 0 5 5 0 0 1-2.5 8.5"/><path d="M12 12v.01"/></svg>
  ),
  Heart: () => (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.505 4.04 3 5.5l7 7Z"/></svg>
  ),
  Activity: () => (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>
  ),
  Settings: () => (
    <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>
  )
};
