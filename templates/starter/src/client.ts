import { io, Socket } from "socket.io-client";
import * as readline from "readline";
import { config } from "./config";

const WEBSOCKET_PORT = parseInt(process.env.WEBSOCKET_PORT || "3001", 10);
const WEBSOCKET_URL =
  process.env.WEBSOCKET_URL || `http://localhost:${WEBSOCKET_PORT}`;

interface AgentMessage {
  type: "system" | "agent" | "error";
  content: string;
  metadata?: Record<string, unknown>;
}

/**
 * Create a CLI interface for user input
 */
function createCLI(): readline.Interface {
  return readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });
}

/**
 * Connect to the agent server
 */
function connectToServer(): Socket {
  const socket = io(WEBSOCKET_URL);

  socket.on("connect", () => {
    console.log("Connected to agent server");
  });

  socket.on("disconnect", () => {
    console.log("\nDisconnected from agent server");
    process.exit(0);
  });

  socket.on("connect_error", (error: Error) => {
    console.error("Connection error:", error.message);
    process.exit(1);
  });

  return socket;
}

/**
 * Start the CLI client
 */
async function main() {
  const socket = connectToServer();
  const rl = createCLI();

  // Handle incoming messages from the server
  socket.on("message", (data: AgentMessage) => {
    if (data.type === "system") {
      console.log("\n[System]:", data.content);
    } else if (data.type === "agent") {
      console.log("\n[Agent]:", data.content);
    } else if (data.type === "error") {
      console.error("\n[Error]:", data.content);
    }
    process.stdout.write("\n> ");
  });

  // Handle user input
  const question = (prompt: string): Promise<string> =>
    new Promise((resolve) => rl.question(prompt, resolve));

  try {
    while (true) {
      const input = await question("> ");

      if (input.toLowerCase() === "exit") {
        break;
      }

      socket.emit("message", { message: input });
    }
  } catch (error) {
    console.error(
      "Error:",
      error instanceof Error ? error.message : String(error)
    );
  } finally {
    rl.close();
    socket.disconnect();
  }
}

// Handle graceful shutdown
process.on("SIGINT", () => {
  console.log("\nShutting down...");
  process.exit(0);
});

// Start the client
if (require.main === module) {
  main().catch((error) => {
    console.error(
      "Fatal error:",
      error instanceof Error ? error.message : String(error)
    );
    process.exit(1);
  });
}

export { connectToServer };
