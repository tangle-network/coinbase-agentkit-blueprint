import * as dotenv from "dotenv";
import { envSchema, type EnvVars, type AgentConfig } from "./types";

// Load environment variables
dotenv.config();

/**
 * Validate and load environment variables
 */
export function loadConfig(): EnvVars {
  const result = envSchema.safeParse(process.env);

  if (!result.success) {
    console.error("❌ Invalid environment variables:");
    console.error(result.error.format());
    process.exit(1);
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
