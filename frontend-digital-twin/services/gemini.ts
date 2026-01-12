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

/**
 * Generate tactical image using OpenRouter API
 * Note: OpenRouter routes to image generation models (DALL-E, Stable Diffusion, etc.)
 * The exact API may vary - this implementation tries the most common approaches
 */
const generateImageViaOpenRouter = async (prompt: string): Promise<string | null> => {
  const apiKey = (import.meta.env.VITE_OPENROUTER_API_KEY || process.env.VITE_OPENROUTER_API_KEY) as string | undefined;
  
  if (!apiKey) {
    console.warn("OpenRouter API key not found. Set VITE_OPENROUTER_API_KEY in .env.local");
    return null;
  }

  const enhancedPrompt = `High-tech cybersecurity tactical visualization: ${prompt}`;

  // Try DALL-E 3 via OpenRouter (if supported)
  try {
    const response = await fetch('https://openrouter.ai/api/v1/chat/completions', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${apiKey}`,
        'Content-Type': 'application/json',
        'HTTP-Referer': window.location.origin,
        'X-Title': 'PAGI Digital Twin'
      },
      body: JSON.stringify({
        model: 'openai/dall-e-3',
        messages: [
          {
            role: 'user',
            content: enhancedPrompt
          }
        ]
      })
    });

    if (response.ok) {
      const data = await response.json();
      
      // Check various response formats
      if (data.choices?.[0]?.message?.content) {
        const content = data.choices[0].message.content;
        if (content.startsWith('http://') || content.startsWith('https://') || content.startsWith('data:image')) {
          return content;
        }
        // Try parsing as JSON
        try {
          const parsed = JSON.parse(content);
          if (parsed.url) return parsed.url;
          if (parsed.data) return parsed.data;
        } catch {
          // Not JSON
        }
      }
      
      // Check alternative response structure
      if (data.data?.[0]?.url) {
        return data.data[0].url;
      }
    } else {
      // If DALL-E fails, try alternative approach: route through backend orchestrator
      // The orchestrator can handle image generation requests
      console.warn("Direct OpenRouter image generation not available, consider routing through backend");
    }
  } catch (error) {
    console.error("OpenRouter image generation error:", error);
  }
  
  return null;
};

/**
 * Generate tactical image - routes through OpenRouter or Gemini based on availability
 */
export const generateTacticalImage = async (prompt: string): Promise<string | null> => {
  // Try OpenRouter first (preferred for unified API)
  const openRouterKey = (import.meta.env.VITE_OPENROUTER_API_KEY || process.env.VITE_OPENROUTER_API_KEY) as string | undefined;
  
  if (openRouterKey) {
    const result = await generateImageViaOpenRouter(prompt);
    if (result) {
      return result;
    }
    console.warn("OpenRouter image generation failed, falling back to Gemini");
  }
  
  // Fallback to Gemini if OpenRouter is not available or fails
  const geminiKey = (import.meta.env.GEMINI_API_KEY || process.env.API_KEY) as string | undefined;
  if (!geminiKey) {
    console.error("Neither OpenRouter nor Gemini API key found. Cannot generate image.");
    return null;
  }
  
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
    console.error("Gemini image generation error:", error);
    return null;
  }
};

/**
 * Generate video using Replicate API (AnimateDiff)
 * Replicate offers free credits and supports multiple video generation models
 * Uses AnimateDiff for text-to-video generation (no image input required)
 */
export const generateDeepVideo = async (prompt: string): Promise<{ status: string; message: string; videoUrl?: string }> => {
  const apiKey = (import.meta.env.VITE_REPLICATE_API_KEY || process.env.VITE_REPLICATE_API_KEY) as string | undefined;
  
  if (!apiKey) {
    console.warn("Replicate API key not found. Set VITE_REPLICATE_API_KEY in .env.local");
    return { 
      status: 'error', 
      message: 'Replicate API key not configured. Get a free API key from https://replicate.com/account/api-tokens' 
    };
  }

  const enhancedPrompt = `High-tech cybersecurity tactical scenario visualization: ${prompt}`;

  try {
    // Use AnimateDiff for text-to-video (no image required)
    return await generateVideoViaAnimateDiff(enhancedPrompt, apiKey);
  } catch (error) {
    console.error("Video generation error:", error);
    return { 
      status: 'error', 
      message: `Video generation failed: ${error instanceof Error ? error.message : 'Unknown error'}` 
    };
  }
};

/**
 * Generate video using AnimateDiff (text-to-video, no image required)
 * This is the primary method since it works with text prompts directly
 */
const generateVideoViaAnimateDiff = async (prompt: string, apiKey: string): Promise<{ status: string; message: string; videoUrl?: string }> => {
  try {
    const response = await fetch('https://api.replicate.com/v1/predictions', {
      method: 'POST',
      headers: {
        'Authorization': `Token ${apiKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        version: 'lucataco/animate-diff:beecf59a4aee8d81f04fba130acb8e63e0f75f40', // AnimateDiff model for text-to-video
        input: {
          prompt: prompt,
          num_frames: 16, // ~0.6 seconds at 25fps
          guidance_scale: 7.5,
          num_inference_steps: 25,
          height: 512,
          width: 512,
          seed: null // Random seed for variety
        }
      })
    });

    if (!response.ok) {
      const errorText = await response.text();
      return { status: 'error', message: `Replicate API error: ${errorText}` };
    }

    const prediction = await response.json();
    let status = prediction.status;
    let getUrl = prediction.urls?.get;
    
    // Poll for completion (simplified - in production, use webhooks)
    let attempts = 0;
    const maxAttempts = 30; // 60 seconds max
    
    while ((status === 'starting' || status === 'processing') && attempts < maxAttempts) {
      await new Promise(resolve => setTimeout(resolve, 2000));
      attempts++;
      
      if (!getUrl) break;
      
      const statusResponse = await fetch(getUrl, {
        headers: { 'Authorization': `Token ${apiKey}` }
      });
      
      if (statusResponse.ok) {
        const statusData = await statusResponse.json();
        status = statusData.status;
        
        if (status === 'succeeded') {
          const videoUrl = statusData.output?.[0] || statusData.output;
          return { 
            status: 'completed', 
            message: 'Video synthesis completed successfully.',
            videoUrl: videoUrl
          };
        } else if (status === 'failed') {
          return { 
            status: 'error', 
            message: `Video generation failed: ${statusData.error || 'Unknown error'}` 
          };
        }
      }
    }
    
    return { status: 'error', message: 'Video generation timed out' };
  } catch (error) {
    return { 
      status: 'error', 
      message: `AnimateDiff error: ${error instanceof Error ? error.message : 'Unknown error'}` 
    };
  }
};

/**
 * Generate code patch using OpenRouter API
 * Uses code-focused models for generating security patches, scripts, and code solutions
 */
export const generateCodePatch = async (prompt: string): Promise<{ status: string; message: string; code?: string; language?: string }> => {
  const apiKey = (import.meta.env.VITE_OPENROUTER_API_KEY || process.env.VITE_OPENROUTER_API_KEY) as string | undefined;
  
  if (!apiKey) {
    console.warn("OpenRouter API key not found. Set VITE_OPENROUTER_API_KEY in .env.local");
    return { 
      status: 'error', 
      message: 'OpenRouter API key not configured. Set VITE_OPENROUTER_API_KEY in .env.local' 
    };
  }

  // Enhanced prompt for code generation with security focus
  const systemPrompt = `You are an expert security-focused code generator. Generate clean, secure, and well-documented code based on the user's requirements. 
- Always include proper error handling
- Follow security best practices
- Add comments explaining the logic
- Use appropriate language for the task
- If the request is for a patch, provide a complete, working solution`;

  const enhancedPrompt = `Generate code for: ${prompt}\n\nProvide the complete code solution with appropriate language syntax highlighting. Include any necessary imports, error handling, and documentation.`;

  try {
    const response = await fetch('https://openrouter.ai/api/v1/chat/completions', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${apiKey}`,
        'Content-Type': 'application/json',
        'HTTP-Referer': window.location.origin,
        'X-Title': 'PAGI Digital Twin - Code Generation'
      },
      body: JSON.stringify({
        model: 'anthropic/claude-3.5-sonnet', // Excellent for code generation
        messages: [
          {
            role: 'system',
            content: systemPrompt
          },
          {
            role: 'user',
            content: enhancedPrompt
          }
        ],
        temperature: 0.2, // Lower temperature for more deterministic code
        max_tokens: 4000 // Allow for longer code blocks
      })
    });

    if (!response.ok) {
      const errorText = await response.text();
      console.error("OpenRouter code generation error:", errorText);
      
      // Try fallback to Gemini if Claude fails
      return await generateCodePatchViaGemini(prompt);
    }

    const data = await response.json();
    
    // Extract code from response
    const content = data.choices?.[0]?.message?.content || '';
    
    if (!content) {
      return { 
        status: 'error', 
        message: 'No code generated in response' 
      };
    }

    // Detect language from code blocks or content
    let language = 'text';
    const codeBlockMatch = content.match(/```(\w+)?/);
    if (codeBlockMatch && codeBlockMatch[1]) {
      language = codeBlockMatch[1];
    } else if (content.includes('def ') || content.includes('import ')) {
      language = 'python';
    } else if (content.includes('fn ') || content.includes('use ') || content.includes('pub ')) {
      language = 'rust';
    } else if (content.includes('function ') || content.includes('const ') || content.includes('let ')) {
      language = 'javascript';
    } else if (content.includes('package ') || content.includes('func ')) {
      language = 'go';
    }

    // Extract code from markdown code blocks if present
    let code = content;
    if (content.includes('```')) {
      const codeBlockRegex = /```(?:\w+)?\n([\s\S]*?)```/;
      const match = content.match(codeBlockRegex);
      if (match && match[1]) {
        code = match[1].trim();
      }
    }

    return { 
      status: 'completed', 
      message: 'Code patch generated successfully.',
      code: code,
      language: language
    };
  } catch (error) {
    console.error("OpenRouter code generation error:", error);
    // Try fallback to Gemini
    return await generateCodePatchViaGemini(prompt);
  }
};

/**
 * Fallback code generation using Gemini API
 */
const generateCodePatchViaGemini = async (prompt: string): Promise<{ status: string; message: string; code?: string; language?: string }> => {
  const geminiKey = (import.meta.env.GEMINI_API_KEY || process.env.API_KEY) as string | undefined;
  
  if (!geminiKey) {
    return { 
      status: 'error', 
      message: 'Neither OpenRouter nor Gemini API key found. Cannot generate code.' 
    };
  }

  try {
    const response = await ai.models.generateContent({
      model: 'gemini-2.0-flash-exp',
      contents: {
        parts: [{ 
          text: `Generate secure, well-documented code for: ${prompt}\n\nProvide complete code with proper error handling, security best practices, and comments.` 
        }]
      },
      config: {
        systemInstruction: 'You are an expert code generator. Generate clean, secure, and well-documented code.',
        temperature: 0.2
      }
    });

    const content = response.text || '';
    
    if (!content) {
      return { 
        status: 'error', 
        message: 'No code generated in Gemini response' 
      };
    }

    // Detect language
    let language = 'text';
    if (content.includes('def ') || content.includes('import ')) {
      language = 'python';
    } else if (content.includes('fn ') || content.includes('use ')) {
      language = 'rust';
    } else if (content.includes('function ') || content.includes('const ')) {
      language = 'javascript';
    }

    // Extract code from markdown blocks if present
    let code = content;
    if (content.includes('```')) {
      const codeBlockRegex = /```(?:\w+)?\n([\s\S]*?)```/;
      const match = content.match(codeBlockRegex);
      if (match && match[1]) {
        code = match[1].trim();
      }
    }

    return { 
      status: 'completed', 
      message: 'Code patch generated successfully (via Gemini fallback).',
      code: code,
      language: language
    };
  } catch (error) {
    console.error("Gemini code generation error:", error);
    return { 
      status: 'error', 
      message: `Code generation failed: ${error instanceof Error ? error.message : 'Unknown error'}` 
    };
  }
};
