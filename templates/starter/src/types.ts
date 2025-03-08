import { z } from "zod";
import { HumanMessage } from "@langchain/core/messages";
import { BaseMessage } from "@langchain/core/messages";

// Environment variable schema
export const envSchema = z.object({
  OPENAI_API_KEY: z.string(),
  CDP_API_KEY_NAME: z.string().optional(),
  CDP_API_KEY_PRIVATE_KEY: z.string().optional(),
  PORT: z.string().default("3000"),
  WEBSOCKET_PORT: z.string().default("3001"),
  WEBSOCKET_URL: z.string().optional(),
  AGENT_MODE: z.enum(["http", "cli-chat"]).default("http"),
  MODEL: z.string().default("gpt-4o-mini"),
  LOG_LEVEL: z.enum(["error", "warn", "info", "debug"]).default("info"),
  NODE_ENV: z
    .enum(["development", "production", "test"])
    .default("development"),
});

export type EnvVars = z.infer<typeof envSchema>;

// Agent configuration
export interface AgentConfig {
  model: string;
  mode: "http" | "cli-chat";
  port: number;
  websocketPort: number;
  websocketUrl?: string;
}

// Agent response types
export interface AgentResponse {
  response: string;
  metadata?: Record<string, unknown>;
}

export interface AgentStatus {
  status: "running" | "error";
  uptime: number;
  mode: string;
  lastError?: string;
}

// Strong typing for LangChain/LangGraph agents
export interface LangChainAgentConfig {
  configurable: {
    thread_id: string;
    [key: string]: unknown;
  };
}

export interface InvokeParams {
  messages: BaseMessage[];
  config?: Record<string, unknown>;
}

// Define interfaces with proper typing for LangGraph agents
export interface LangChainAgent {
  // Modern agents use the stream method
  stream?: (
    input: {
      messages: BaseMessage[];
    },
    config?: Record<string, unknown>
  ) => AsyncIterable<{
    agent?: { messages: { content: string }[] };
    tools?: { messages: { content: string }[] };
    [key: string]: any;
  }>;

  // Older agents may use invoke method
  invoke?: (
    input: {
      messages: BaseMessage[];
    },
    config?: Record<string, unknown>
  ) => Promise<any>;

  // Legacy agents might use chat method
  chat?: (input: string) => Promise<string>;

  // Allow for additional properties
  [key: string]: any;
}

export interface InitializeResult {
  agent: LangChainAgent;
  config: LangChainAgentConfig;
}
