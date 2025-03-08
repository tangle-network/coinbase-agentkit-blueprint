import { AgentKit } from "@coinbase/agentkit";

// Mock AgentKit instance
export class MockAgentKit {
  static async from() {
    return {
      getTools: async () => [],
    } as unknown as AgentKit;
  }
}

// Mock LLM class that returns predictable responses
export class MockLLM {
  responseTemplate: string;

  constructor(responseTemplate = "This is a mock response.") {
    this.responseTemplate = responseTemplate;
  }

  async chat(message: string) {
    return this.responseTemplate.replace("[MESSAGE]", message);
  }

  async invoke() {
    return { content: this.responseTemplate };
  }
}

// Mock agent object
export const createMockAgent = (
  responseTemplate = "This is a mock response."
) => {
  return {
    chat: async (message: string) => {
      return responseTemplate.replace("[MESSAGE]", message);
    },
  };
};

// Mock createReactAgent
export const createMockReactAgent = async () => {
  return createMockAgent();
};

// Create a mock environment for testing
export const setupTestEnv = () => {
  // Save original environment
  const originalEnv = { ...process.env };

  // Set up test environment variables
  process.env.OPENAI_API_KEY = "test-openai-api-key";
  process.env.MODEL = "gpt-4o-mini";
  process.env.AGENT_MODE = "http";
  process.env.PORT = "3000";
  process.env.WEBSOCKET_PORT = "3001";

  // Function to restore original environment
  const restoreEnv = () => {
    process.env = originalEnv;
  };

  return { restoreEnv };
};
