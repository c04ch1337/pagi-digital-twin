import { GoogleGenAI } from "@google/genai";
import { Twin } from "../types";
import { VectorShard, getNamespaceShards } from "./memory";

const ai = new GoogleGenAI({ apiKey: process.env.API_KEY });

export const generateAgentResponse = async (
  twin: Twin,
  prompt: string,
  history: { role: 'user' | 'assistant', content: string }[]
) => {
  const model = twin.isOrchestrator ? 'gemini-3-pro-preview' : 'gemini-3-flash-preview';
  
  // Retrieve relevant context from local memory if available
  const shards = getNamespaceShards(twin.settings.memoryNamespace);
  const contextBlock = shards.length > 0 
    ? `\n\n# RELEVANT TACTICAL MEMORY (Namespace: ${twin.settings.memoryNamespace}):\n${shards.map(s => `- [${s.timestamp.toISOString()}] ${s.text}`).join('\n')}`
    : "";

  let policyConstraints = "";
  if (!twin.settings.aiCodeGenerationEnabled) {
    policyConstraints += "\n\n# SECURITY POLICY: CODE GENERATION RESTRICTED\nDO NOT output code blocks.";
  }
  if (twin.settings.safeMode) {
    policyConstraints += "\n\n# SECURITY POLICY: SANDBOX ACTIVE\nOnly propose isolated solutions.";
  }

  const contents = [
    ...history.map(h => ({
      role: h.role === 'user' ? 'user' : 'model',
      parts: [{ text: h.content }]
    })),
    {
      role: 'user',
      parts: [{ text: prompt }]
    }
  ];

  try {
    const response = await ai.models.generateContent({
      model,
      contents,
      config: {
        systemInstruction: twin.systemPrompt + policyConstraints + contextBlock,
        thinkingConfig: { thinkingBudget: twin.isOrchestrator ? 10000 : 5000 },
        tools: [{ googleSearch: {} }]
      }
    });

    return {
      text: response.text || "Tactical agent unresponsive.",
      grounding: response.candidates?.[0]?.groundingMetadata?.groundingChunks || []
    };
  } catch (error) {
    console.error("Gemini API Error:", error);
    throw error;
  }
};

/**
 * Performs semantic search over a namespace's memory shards using Gemini as the vector engine.
 */
export const querySemanticMemory = async (namespace: string, query: string): Promise<VectorShard[]> => {
  const shards = getNamespaceShards(namespace);
  if (shards.length === 0) return [];

  try {
    // We use a specific prompt to find the most relevant shards
    const response = await ai.models.generateContent({
      model: 'gemini-3-flash-preview',
      contents: `You are a semantic retrieval engine. I have a query: "${query}"
      
      Here is a list of knowledge shards from a vector database (Format: ID: Text):
      ${shards.map(s => `${s.id}: ${s.text}`).join('\n')}
      
      Identify the IDs of the shards that are semantically relevant to the query. 
      Return only the IDs as a comma-separated list, or "NONE" if nothing matches.`,
    });

    const relevantIds = response.text?.split(',').map(id => id.trim()) || [];
    return shards.filter(s => relevantIds.includes(s.id));
  } catch (error) {
    console.error("Semantic Search Error:", error);
    return [];
  }
};

export const generateTacticalImage = async (prompt: string) => {
  try {
    const response = await ai.models.generateContent({
      model: 'gemini-2.5-flash-image',
      contents: {
        parts: [{ text: `High-tech cybersecurity tactical visualization: ${prompt}` }]
      },
      config: {
        imageConfig: { aspectRatio: "16:9" }
      }
    });

    if (response.candidates?.[0]?.content?.parts) {
      for (const part of response.candidates[0].content.parts) {
        if (part.inlineData) {
          return `data:image/png;base64,${part.inlineData.data}`;
        }
      }
    }
    return null;
  } catch (error) {
    console.error("Image Gen Error:", error);
    return null;
  }
};

export const generateDeepVideo = async (prompt: string) => {
  return { status: 'initiated', message: 'Video synthesis in progress.' };
};
