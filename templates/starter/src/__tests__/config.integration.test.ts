import { spawn, ChildProcess } from "child_process";
import fs from "fs";
import path from "path";
import { once } from "events";
import request from "supertest";
import { config } from "../config";

describe("Configuration Tests", () => {
  let originalEnv: NodeJS.ProcessEnv;

  beforeEach(() => {
    // Save original environment
    originalEnv = process.env;
    // Reset the require cache for config.ts
    jest.resetModules();
  });

  afterEach(() => {
    // Restore original environment
    process.env = originalEnv;
  });

  it("should load default configuration values", () => {
    // Clear relevant environment variables
    delete process.env.PORT;
    delete process.env.AGENT_MODE;
    delete process.env.OPENAI_API_KEY;

    // Reimport the config
    const { config } = require("../config");

    // Check that defaults are set
    expect(config.PORT).toBe(3000);
    expect(config.AGENT_MODE).toBe("http");
  });

  it("should override defaults with environment variables", () => {
    // Set environment variables
    process.env.PORT = "4000";
    process.env.AGENT_MODE = "cli-chat";
    process.env.OPENAI_API_KEY = "test-key-123";

    // Reimport the config
    const { config } = require("../config");

    // Check that environment variables are used
    expect(config.PORT).toBe(4000);
    expect(config.AGENT_MODE).toBe("cli-chat");
    expect(config.OPENAI_API_KEY).toBe("test-key-123");
  });

  it("should validate PORT as a number", () => {
    // Set invalid PORT
    process.env.PORT = "not-a-number";

    // Expect error when importing config
    expect(() => {
      require("../config");
    }).toThrow();
  });

  it("should validate AGENT_MODE as a valid option", () => {
    // Set invalid AGENT_MODE
    process.env.AGENT_MODE = "invalid-mode";

    // Expect error when importing config
    expect(() => {
      require("../config");
    }).toThrow();
  });

  it("should load configuration from .env file", async () => {
    // Create a test .env file
    const testEnvPath = path.join(__dirname, "../../.env.test");
    fs.writeFileSync(
      testEnvPath,
      `
PORT=5000
AGENT_MODE=http
OPENAI_API_KEY=test-env-file-key
WEBSOCKET_PORT=5001
    `
    );

    // Start a process that loads from this .env file
    const proc = spawn("node", [
      "-e",
      `
require('dotenv').config({ path: "${testEnvPath}" });
const { config } = require("${path.join(__dirname, "../config")}");
console.log(JSON.stringify(config));
    `,
    ]);

    let output = "";
    proc.stdout.on("data", (data) => {
      output += data.toString();
    });

    // Wait for process to exit
    await once(proc, "exit");

    // Parse the config output
    const parsedConfig = JSON.parse(output);

    // Check values loaded from .env file
    expect(parsedConfig.PORT).toBe(5000);
    expect(parsedConfig.AGENT_MODE).toBe("http");
    expect(parsedConfig.OPENAI_API_KEY).toBe("test-env-file-key");
    expect(parsedConfig.WEBSOCKET_PORT).toBe(5001);

    // Clean up
    fs.unlinkSync(testEnvPath);
  });
});

describe("Server Configuration Integration Tests", () => {
  let server: any;
  let testApp: any;
  let originalEnv: NodeJS.ProcessEnv;

  beforeEach(() => {
    // Save original environment
    originalEnv = process.env;
  });

  afterEach(() => {
    // Clean up server if it exists
    if (server && server.close) {
      server.close();
    }
    // Restore original environment
    process.env = originalEnv;
  });

  it("should start server with correct PORT from environment", async () => {
    // Set test port
    const TEST_PORT = 4567;
    process.env.PORT = TEST_PORT.toString();
    process.env.AGENT_MODE = "http";
    process.env.OPENAI_API_KEY = "test-key-123";
    process.env.MOCK_AGENT = "true"; // Use mock agent for testing

    // Dynamically import to get fresh config
    const { startServer } = require("../index");

    // Start server
    testApp = await startServer();

    // Test that server is listening on the correct port
    const response = await request(`http://localhost:${TEST_PORT}`)
      .get("/health")
      .timeout(5000);

    expect(response.status).toBe(200);
    expect(response.body).toHaveProperty("status", "ok");
  });

  it("should apply different configuration in test environment", async () => {
    // Set test environment
    process.env.NODE_ENV = "test";
    process.env.AGENT_MODE = "http";
    process.env.PORT = "4568";
    process.env.MOCK_AGENT = "true"; // Use mock agent for testing

    // Dynamically import to get fresh config with test settings
    const { startServer } = require("../index");

    // Start server
    testApp = await startServer();

    // Check status endpoint which should reflect test configuration
    const response = await request(`http://localhost:4568`)
      .get("/status")
      .timeout(5000);

    expect(response.status).toBe(200);
    expect(response.body).toHaveProperty("status", "running");
    // In test mode, we expect some indication in the status (depends on implementation)
    // This is a placeholder assertion - adjust based on your actual implementation
    expect(response.body).toHaveProperty("mode");
  });

  it("should handle invalid configuration gracefully", async () => {
    // Use the spawn API to start a new process with bad config
    const testProcess = spawn("node", [
      "-e",
      `
      process.env.PORT = "not-a-port";
      process.env.AGENT_MODE = "http";
      
      try {
        require("${path.join(__dirname, "../index")}");
        console.log("Server started (should not happen)");
        process.exit(0);
      } catch (error) {
        console.error("Error:", error.message);
        process.exit(1);
      }
      `,
    ]);

    let stderr = "";
    testProcess.stderr.on("data", (data) => {
      stderr += data.toString();
    });

    // Wait for process to exit
    const [exitCode] = await once(testProcess, "exit");

    // Should exit with error
    expect(exitCode).toBe(1);
    // Error should mention configuration
    expect(stderr).toContain("Error");
  });
});
