import { GoogleGenAI, Type } from "@google/genai";
import { Job, Twin, LogEntry } from "../types";

const ai = new GoogleGenAI({ apiKey: process.env.API_KEY });

export interface ExecutionStep {
  message: string;
  progress: number;
  level: 'info' | 'warn' | 'error' | 'plan' | 'tool' | 'memory';
  delayMs: number;
}

/**
 * Tactical Orchestrator Service
 * Manages the simulation of job execution using AI-generated log sequences.
 */
export const executeJobLifecycle = async (
  job: Job,
  twin: Twin,
  onUpdate: (updatedJob: Job) => void
) => {
  try {
    // Phase 1: Planning
    // We use Gemini 3 Flash to "think" of realistic steps for this specific job
    const response = await ai.models.generateContent({
      model: 'gemini-3-flash-preview',
      contents: `Generate a realistic sequence of tactical execution logs for a security agent mission.
      Agent Role: ${twin.role}
      Mission: ${job.name}
      Namespace: ${twin.settings.memoryNamespace}

      Return a JSON array of steps. Each step must have:
      - "message": A technical, high-precision log message.
      - "progress": The cumulative progress (0-100).
      - "level": One of: info, warn, error, plan, tool, memory.
      - "delayMs": Time in milliseconds to wait before this step (between 500 and 3000).`,
      config: {
        responseMimeType: "application/json",
        responseSchema: {
          type: Type.ARRAY,
          items: {
            type: Type.OBJECT,
            properties: {
              message: { type: Type.STRING },
              progress: { type: Type.NUMBER },
              level: { type: Type.STRING },
              delayMs: { type: Type.NUMBER }
            },
            required: ["message", "progress", "level", "delayMs"]
          }
        }
      }
    });

    const steps: ExecutionStep[] = JSON.parse(response.text || "[]");
    
    // Initial active state
    let currentJob = { ...job, status: 'active' as const, progress: 0, logs: job.logs || [] };
    onUpdate(currentJob);

    // Phase 2: Execution
    for (const step of steps) {
      await new Promise(resolve => setTimeout(resolve, step.delayMs));
      
      const newLog: LogEntry = {
        id: `log-${Math.random().toString(36).substr(2, 9)}`,
        timestamp: new Date(),
        level: step.level,
        message: step.message
      };

      currentJob = {
        ...currentJob,
        progress: step.progress,
        logs: [...currentJob.logs!, newLog]
      };
      
      onUpdate(currentJob);
    }

    // Phase 3: Completion
    await new Promise(resolve => setTimeout(resolve, 1000));
    onUpdate({
      ...currentJob,
      status: 'completed',
      progress: 100,
      logs: [...currentJob.logs!, {
        id: `log-final-${Date.now()}`,
        timestamp: new Date(),
        level: 'info',
        message: `Mission sequence finalized. ${job.name} completed successfully.`
      }]
    });

  } catch (error) {
    console.error("Job Execution Error:", error);
    onUpdate({
      ...job,
      status: 'failed',
      logs: [...(job.logs || []), {
        id: `log-error-${Date.now()}`,
        timestamp: new Date(),
        level: 'error',
        message: `Critical failure in job orchestrator: ${error instanceof Error ? error.message : 'Unknown error'}`
      }]
    });
  }
};
