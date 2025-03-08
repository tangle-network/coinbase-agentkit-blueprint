import { jest } from "@jest/globals";
import { Agent } from "../agent";
import { createMockAgent, setupTestEnv } from "./mockUtils";

// Mock dependencies
jest.mock("@coinbase/agentkit", () => ({
  AgentKit: {
    from: jest.fn().mockImplementation(() => ({
      getTools: jest.fn().mockResolvedValue([] as never),
    })),
  },
}));

jest.mock("@langchain/openai", () => ({
  ChatOpenAI: jest.fn().mockImplementation(() => ({
    invoke: jest.fn().mockResolvedValue({ content: "Mock response" } as never),
  })),
}));

jest.mock("@langchain/langgraph/prebuilt", () => ({
  createReactAgent: jest
    .fn()
    .mockImplementation(() => createMockAgent("Mock agent response")),
}));

describe("Agent", () => {
  const { restoreEnv } = setupTestEnv();
  let agent: Agent;

  beforeEach(async () => {
    agent = new Agent();
    await agent.initialize();
  });

  afterAll(() => {
    restoreEnv();
    jest.clearAllMocks();
  });

  it("should initialize correctly", () => {
    expect(agent).toBeDefined();
    expect(agent.agent).toBeDefined();
    expect(agent.agentConfig).toBeDefined();
  });

  it("should process messages and return responses", async () => {
    const response = await agent.processMessage("Hello agent");
    expect(response).toBeDefined();
    expect(response.response).toBe("Mock agent response");
    expect(response.metadata).toBeDefined();
    expect(response.metadata.agentConfig).toBeDefined();
  });

  it("should return status information", () => {
    const status = agent.getStatus();
    expect(status).toBeDefined();
    expect(status.status).toBe("running");
    expect(status.uptime).toBeGreaterThanOrEqual(0);
    expect(status.mode).toBeDefined();
  });

  it("should handle errors when processing messages", async () => {
    // Override the mock to throw an error
    jest.spyOn(agent.agent, "chat").mockImplementationOnce(() => {
      throw new Error("Test error");
    });

    await expect(agent.processMessage("Hello")).rejects.toThrow(
      "Failed to process message"
    );
  });
});
