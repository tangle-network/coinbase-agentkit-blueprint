import * as dotenv from "dotenv";
import { envSchema, type EnvVars, type AgentConfig } from "./types";

// Load environment variables
dotenv.config();

/**
 * Validate and load environment variables
 */
export function loadConfig(): EnvVars {
  // In test environments, provide default values
  if (process.env.NODE_ENV === "test") {
    process.env.OPENAI_API_KEY = process.env.OPENAI_API_KEY || "test-api-key";
    process.env.AGENT_MODE = process.env.AGENT_MODE || "http";
    process.env.PORT = process.env.PORT || "3000";
    process.env.WEBSOCKET_PORT = process.env.WEBSOCKET_PORT || "3001";
  }

  const result = envSchema.safeParse(process.env);

  if (!result.success) {
    console.error("❌ Invalid environment variables:");
    console.error(result.error.format());

    // In test environments, don't exit the process
    if (process.env.NODE_ENV !== "test") {
      process.exit(1);
    } else {
      console.warn("⚠️ Running with default test values");
      // Provide default test config as fallback
      return {
        OPENAI_API_KEY: "test-api-key",
        PORT: "3000",
        WEBSOCKET_PORT: "3001",
        AGENT_MODE: "http",
        MODEL: "gpt-4o-mini",
        LOG_LEVEL: "info",
        NODE_ENV: "test",
      } as EnvVars;
    }
  }

  return result.data;
}

/**
 * Create agent configuration from environment variables
 */
export function createAgentConfig(env: EnvVars): AgentConfig {
  return {
    model: env.MODEL,
    mode: env.AGENT_MODE,
    port: parseInt(env.PORT, 10),
    websocketPort: parseInt(env.WEBSOCKET_PORT, 10),
    websocketUrl: env.WEBSOCKET_URL,
  };
}

// Export validated config
export const config = loadConfig();
export const agentConfig = createAgentConfig(config);
