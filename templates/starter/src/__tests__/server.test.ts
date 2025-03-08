/**
 * Real server test that launches the actual HTTP server
 * and tests it by sending requests to it
 */

import request from "supertest";
import * as dotenv from "dotenv";
import { spawn, ChildProcess } from "child_process";
import { setTimeout } from "timers/promises";

// Load environment variables
dotenv.config();

// Only run these tests if explicitly enabled
// These tests can be flaky as they rely on starting a real server
const runServerTests = process.env.RUN_SERVER_TESTS === "true";
(runServerTests ? describe : describe.skip)("Live Server Test", () => {
  let serverProcess: ChildProcess;
  const PORT = 3099; // Use a different port for testing

  beforeAll(async () => {
    // Override environment variables for testing
    process.env.PORT = PORT.toString();
    process.env.AGENT_MODE = "http";
    process.env.NODE_ENV = "test";

    // Start the actual server as a separate process
    console.log("Starting server process...");
    serverProcess = spawn("ts-node", ["src/index.ts"], {
      env: {
        ...process.env,
        PORT: PORT.toString(),
        NODE_ENV: "test",
        OPENAI_API_KEY: process.env.OPENAI_API_KEY || "test-api-key",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    // Log server output for debugging
    serverProcess.stdout?.on("data", (data) => {
      console.log(`Server stdout: ${data}`);
    });

    serverProcess.stderr?.on("data", (data) => {
      console.error(`Server stderr: ${data}`);
    });

    // Wait for server to start
    await setTimeout(3000);
    console.log("Server should be ready now");
  }, 10000); // 10s timeout for server startup

  afterAll(() => {
    // Clean up server process
    if (serverProcess) {
      console.log("Terminating server process");
      serverProcess.kill();
    }
  });

  test("Server health check responds correctly", async () => {
    try {
      const response = await request(`http://localhost:${PORT}`).get("/health");
      expect(response.status).toBe(200);
      expect(response.body.status).toBe("ok");
    } catch (error) {
      console.error("Error testing health check:", error);
      throw error;
    }
  });

  test("Server status endpoint responds correctly", async () => {
    try {
      const response = await request(`http://localhost:${PORT}`).get("/status");
      expect(response.status).toBe(200);
      expect(response.body.status).toBe("running");
    } catch (error) {
      console.error("Error testing status endpoint:", error);
      throw error;
    }
  });

  // Skip the interaction test by default as it requires a real OpenAI API key
  (process.env.OPENAI_API_KEY && process.env.OPENAI_API_KEY !== "test-api-key"
    ? test
    : test.skip)(
    "Server can process an interaction request",
    async () => {
      try {
        const response = await request(`http://localhost:${PORT}`)
          .post("/interact")
          .send({ message: "Hello, agent!" })
          .set("Accept", "application/json");

        expect(response.status).toBe(200);
        expect(response.body.response).toBeDefined();

        // Log the response for verification
        console.log("Live server response:", response.body.response);
      } catch (error) {
        console.error("Error testing interaction:", error);
        throw error;
      }
    },
    20000
  ); // 20s timeout for LLM response
});
