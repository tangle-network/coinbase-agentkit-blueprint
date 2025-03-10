import { io } from "socket.io-client";
import { spawn, ChildProcess } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as dotenv from "dotenv";

// Load environment variables from .env file
dotenv.config();

// Skip tests if required dependencies are not available
let canRunTests = true;
try {
  require("socket.io-client");
} catch (e) {
  canRunTests = false;
}

// Only run these tests if dependencies are available
const itIfDeps = canRunTests ? it : it.skip;

describe("Real WebSocket Tests", () => {
  let serverProcess: ChildProcess;
  let client: any;
  const WS_PORT = 3567; // Use a unique port for testing
  const TEST_TIMEOUT = 60000; // Increase timeout for real API calls

  // Store event listeners so we can remove them later
  let stdoutListener: (data: Buffer) => void;
  let stderrListener: (data: Buffer) => void;

  beforeAll(async () => {
    if (!canRunTests) {
      console.warn("socket.io-client not available, skipping WebSocket tests");
      return;
    }

    // Get the actual API key from environment or .env file
    const API_KEY = process.env.OPENAI_API_KEY;

    if (!API_KEY || API_KEY === "your_openai_api_key_here") {
      console.warn("No valid OpenAI API key found, skipping real tests");
      return;
    }

    // Start agent in WebSocket mode in a separate process with real API key
    console.log("Starting WebSocket server on port", WS_PORT);
    serverProcess = spawn("ts-node", ["--transpile-only", "src/index.ts"], {
      env: {
        ...process.env,
        AGENT_MODE: "cli-chat",
        WEBSOCKET_PORT: WS_PORT.toString(),
        NODE_ENV: "test",
        PORT: "3000",
        // Use the actual OpenAI API key from .env
        OPENAI_API_KEY: API_KEY,
      },
      stdio: "pipe",
    });

    // Log server output for debugging
    stdoutListener = (data: Buffer) => {
      console.log(`[SERVER]: ${data.toString().trim()}`);
    };

    stderrListener = (data: Buffer) => {
      console.error(`[SERVER ERROR]: ${data.toString().trim()}`);
    };

    serverProcess.stdout?.on("data", stdoutListener);
    serverProcess.stderr?.on("data", stderrListener);

    // Wait for server to start
    console.log("Waiting for server to start...");
    await new Promise((resolve) => setTimeout(resolve, 5000));

    // Create socket.io client
    console.log(
      `Connecting to WebSocket server at http://localhost:${WS_PORT}`
    );
    client = io(`http://localhost:${WS_PORT}`, {
      transports: ["websocket"],
      reconnectionAttempts: 5,
      reconnectionDelay: 1000,
    });

    // Wait for connection with improved error handling
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(
        () => reject(new Error("Connection timeout after 5000ms")),
        5000
      );

      client.on("connect", () => {
        console.log("WebSocket client connected successfully");
        clearTimeout(timeout);
        resolve();
      });

      client.on("connect_error", (err: Error) => {
        console.error("WebSocket connection error:", err.message);
        // Don't reject here, let the timeout handle it
      });
    });
  }, TEST_TIMEOUT);

  afterAll(() => {
    // First remove event listeners to prevent logging after test completion
    if (serverProcess?.stdout && stdoutListener) {
      serverProcess.stdout.removeListener("data", stdoutListener);
    }

    if (serverProcess?.stderr && stderrListener) {
      serverProcess.stderr.removeListener("data", stderrListener);
    }

    // Clean up
    if (client) {
      console.log("Disconnecting WebSocket client");
      client.disconnect();
    }

    if (serverProcess) {
      console.log("Stopping WebSocket server process");
      serverProcess.kill("SIGTERM");

      // Wait for process to exit gracefully
      serverProcess.unref();
    }
  });

  itIfDeps(
    "should connect to the real WebSocket server",
    () => {
      expect(client.connected).toBe(true);
    },
    TEST_TIMEOUT
  );

  itIfDeps(
    "should receive welcome message",
    (done) => {
      client.once("message", (data: any) => {
        expect(data).toHaveProperty("type", "system");
        expect(data.content).toContain("Connected to agent");
        done();
      });
    },
    TEST_TIMEOUT
  );

  itIfDeps(
    "should process a real message and get a response",
    (done) => {
      const testMessage = "What is 2+2?";

      client.once("message", (response: any) => {
        console.log("Received response:", response);

        expect(response).toHaveProperty("type", "agent");
        expect(response).toHaveProperty("content");
        expect(typeof response.content).toBe("string");
        expect(response.content.length).toBeGreaterThan(0);

        done();
      });

      // Send test message
      client.emit("message", { message: testMessage });
    },
    TEST_TIMEOUT
  );
});
