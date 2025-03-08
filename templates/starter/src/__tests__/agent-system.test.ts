/**
 * Comprehensive agent system tests
 *
 * This file includes both:
 * 1. Mock-based tests that don't require API keys
 * 2. Real API tests that use actual OpenAI API (when enabled via RUN_API_TESTS=true)
 */

import express from "express";
import request from "supertest";
import { Agent } from "../agent";
import * as dotenv from "dotenv";

// Load environment variables
dotenv.config();

// Enable/disable real API tests via environment variable
const RUN_API_TESTS = process.env.RUN_API_TESTS === "true";

// Set test timeout in ms for API tests
const API_TEST_TIMEOUT = 15000;

describe("Agent System Tests (Mock)", () => {
  let agent: Agent;
  let app: express.Application;

  beforeAll(async () => {
    // Set environment for test
    process.env.NODE_ENV = "test";

    // Set up a mocked agent instance
    agent = new Agent();

    // Create a mock _agent property
    const mockAgentChat = jest
      .fn()
      .mockImplementation(async (message: string) => {
        return `Mock response to: ${message}`;
      });

    // Mock the initialize method to set up the mock agent
    agent.initialize = jest.fn().mockImplementation(async () => {
      (agent as any)._agent = {
        chat: mockAgentChat,
        // Add mock invoke method
        invoke: jest
          .fn()
          .mockImplementation(async (input: any, config?: any) => {
            const message =
              typeof input === "string"
                ? input
                : input?.messages?.[0]?.content || "No message content";
            return { content: `Mock response to: ${message}` };
          }),
        // Add mock stream method that returns an async iterable
        stream: jest.fn().mockImplementation(async function* (
          input: any,
          config?: any
        ) {
          const message = input?.messages?.[0]?.content || "No message content";

          // Yield a mock agent response
          yield {
            agent: {
              messages: [{ content: `Mock response to: ${message}` }],
            },
          };

          // Simulate a slight delay
          await new Promise((resolve) => setTimeout(resolve, 10));

          // Yield a mock tool response
          yield {
            tools: {
              messages: [{ content: `Tool response for: ${message}` }],
            },
          };
        }),
      };
      (agent as any)._agentConfig = {
        configurable: {
          thread_id: "Mock Thread",
        },
      };
      (agent as any).startTime = Date.now() - 60000; // 1 minute ago
    });

    // Mock the processMessage method to use the streaming mock agent
    agent.processMessage = jest
      .fn()
      .mockImplementation(async (message: string) => {
        if (!(agent as any)._agent) {
          throw new Error("Agent not initialized");
        }

        // Use stream method if available
        if ((agent as any)._agent.stream) {
          // Extract the first agent message from the stream
          const stream = await (agent as any)._agent.stream(
            { messages: [{ content: message }] },
            (agent as any)._agentConfig.configurable
          );

          // Get the first yielded value (we just want one response for testing)
          const firstResponse = await stream.next();
          const responseContent =
            firstResponse.value?.agent?.messages?.[0]?.content ||
            `Mock response to: ${message}`;

          return {
            response: responseContent,
            metadata: { mocked: true },
          };
        } else {
          // Fallback to chat for older implementation
          const response = await (agent as any)._agent.chat(message);
          return {
            response,
            metadata: { mocked: true },
          };
        }
      });

    // Mock the getStatus method
    agent.getStatus = jest.fn().mockReturnValue({
      status: "running",
      uptime: 60,
      mode: "http",
    });

    // Initialize the agent
    await agent.initialize();

    // Create an Express app for testing
    app = express();
    app.use(express.json());

    // Health check endpoint
    app.get("/health", (_, res) => {
      return res.json({ status: "ok" });
    });

    // Status endpoint
    app.get("/status", (_, res) => {
      return res.json(agent.getStatus());
    });

    // Interaction endpoint
    app.post("/interact", async (req, res) => {
      try {
        const { message } = req.body;
        if (!message || typeof message !== "string") {
          return res.status(400).json({ error: "Invalid message format" });
        }

        const response = await agent.processMessage(message);
        return res.json(response);
      } catch (error) {
        console.error("Error processing message:", error);
        return res.status(500).json({ error: "Failed to process message" });
      }
    });
  });

  test("Health check endpoint returns OK status", async () => {
    const response = await request(app).get("/health");
    expect(response.status).toBe(200);
    expect(response.body.status).toBe("ok");
  });

  test("Status endpoint returns agent status", async () => {
    const response = await request(app).get("/status");
    expect(response.status).toBe(200);
    expect(response.body.status).toBe("running");
    expect(response.body.uptime).toBe(60);
    expect(response.body.mode).toBe("http");
  });

  test("Agent can process a message (mocked)", async () => {
    const message = "Hello, agent!";
    const response = await request(app)
      .post("/interact")
      .send({ message })
      .set("Accept", "application/json");

    expect(response.status).toBe(200);
    expect(response.body.response).toBeDefined();
    expect(response.body.response).toBe(`Mock response to: ${message}`);
    expect(response.body.metadata).toEqual({ mocked: true });

    // Verify the mock was called with the correct arguments
    expect(agent.processMessage).toHaveBeenCalledWith(message);
  });

  test("Agent returns 400 for invalid message format", async () => {
    const response = await request(app)
      .post("/interact")
      .send({ not_a_message: "Should fail" })
      .set("Accept", "application/json");

    expect(response.status).toBe(400);
    expect(response.body.error).toBe("Invalid message format");
  });
});

// Only run these tests if explicitly enabled and API key is available
(RUN_API_TESTS ? describe : describe.skip)(
  "Agent System Tests (Real API)",
  () => {
    let agent: Agent;
    let app: express.Application;

    beforeAll(async () => {
      console.log("Running tests with real OpenAI API...");

      try {
        // Check if we have an API key
        const apiKey = process.env.OPENAI_API_KEY;
        console.log(`API key available: ${!!apiKey}`);
        if (apiKey) {
          console.log(`API key length: ${apiKey.length}`);
        }

        // Set environment for test
        process.env.NODE_ENV = "test";

        // Set up a real agent instance
        agent = new Agent();

        // Initialize with real API calls
        console.log("Initializing agent with real API...");
        await agent.initialize();
        console.log("Agent initialized successfully");

        // Create an Express app for testing
        app = express();
        app.use(express.json());

        // Health check endpoint
        app.get("/health", (_, res) => {
          return res.json({ status: "ok" });
        });

        // Status endpoint
        app.get("/status", (_, res) => {
          try {
            const status = agent.getStatus();
            return res.json(status);
          } catch (error) {
            console.error("Error getting status:", error);
            return res.status(500).json({ error: "Failed to get status" });
          }
        });

        // Interaction endpoint with detailed logging
        app.post("/interact", async (req, res) => {
          console.log("Received interaction request:", req.body);
          try {
            const { message } = req.body;
            if (!message || typeof message !== "string") {
              console.log("Invalid message format");
              return res.status(400).json({ error: "Invalid message format" });
            }

            console.log(`Processing message: ${message}`);
            const response = await agent.processMessage(message);
            console.log("Response from agent:", response);
            return res.json(response);
          } catch (error) {
            console.error("Error processing message:", error);
            return res.status(500).json({
              error: "Failed to process message",
              details: error instanceof Error ? error.message : String(error),
            });
          }
        });
      } catch (error) {
        console.error("Error in test setup:", error);
        throw error;
      }
    }, API_TEST_TIMEOUT);

    test(
      "Agent can process a message with real API",
      async () => {
        try {
          // First verify we can get the status
          const statusResponse = await request(app).get("/status");
          console.log("Status response:", statusResponse.body);

          // Now try to process a message
          console.log("Sending message to agent...");
          const message = "What time is it?";
          const response = await request(app)
            .post("/interact")
            .send({ message })
            .set("Accept", "application/json");

          console.log("Response status:", response.status);
          console.log("Response body:", response.body);

          expect(response.status).toBe(200);
          expect(response.body.response).toBeDefined();

          // The response should be a non-empty string
          expect(typeof response.body.response).toBe("string");
          expect(response.body.response.length).toBeGreaterThan(0);

          // Log the actual response for verification
          console.log("Real API response:", response.body.response);
        } catch (error) {
          console.error("Test error:", error);
          throw error;
        }
      },
      API_TEST_TIMEOUT
    );
  }
);
