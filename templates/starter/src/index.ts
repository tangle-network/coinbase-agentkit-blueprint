import express, { Request, Response } from "express";
import { createServer } from "http";
import { Server, Socket } from "socket.io";
import { Agent } from "./agent";
import { config } from "./config";
import { createLogger } from "./logger";

const logger = createLogger();
const WEBSOCKET_PORT = parseInt(process.env.WEBSOCKET_PORT || "3001", 10);

interface MessageData {
  message: string;
}

/**
 * Start the HTTP server
 */
async function startServer() {
  // Create and initialize agent
  const agent = new Agent();
  await agent.initialize();

  logger.info("Agent initialized successfully");

  // Set up HTTP server
  const app = express();
  app.use(express.json());

  // Health check endpoint
  app.get("/health", (_: Request, res: Response) => {
    res.json({ status: "ok" });
  });

  // Status endpoint
  app.get("/status", (_: Request, res: Response) => {
    res.json(agent.getStatus());
  });

  // Interaction endpoint
  app.post("/interact", async (req: Request, res: Response) => {
    try {
      const { message } = req.body;
      if (!message || typeof message !== "string") {
        return res.status(400).json({ error: "Invalid message format" });
      }

      const response = await agent.processMessage(message);
      return res.json(response);
    } catch (error) {
      logger.error("Error processing message:", error);
      return res.status(500).json({ error: "Failed to process message" });
    }
  });

  // Start server
  app.listen(config.PORT, () => {
    logger.info(`Agent HTTP server listening on port ${config.PORT}`);
  });

  return app;
}

/**
 * Start the WebSocket server for CLI chat mode
 */
async function startWebSocketServer() {
  // Create and initialize agent
  const agent = new Agent();
  await agent.initialize();

  logger.info("Agent initialized successfully");

  // Create WebSocket server
  const httpServer = createServer();
  const io = new Server(httpServer, {
    cors: {
      origin: "*", // In production, configure this to your specific origins
      methods: ["GET", "POST"],
    },
  });

  // Handle WebSocket connections
  io.on("connection", (socket: Socket) => {
    logger.info("New client connected");

    // Send welcome message
    socket.emit("message", {
      type: "system",
      content: "Connected to agent. Type your message to begin.",
    });

    // Handle incoming messages
    socket.on("message", async (data: MessageData) => {
      try {
        const response = await agent.processMessage(data.message);
        socket.emit("message", {
          type: "agent",
          content: response.response,
          metadata: response.metadata,
        });
      } catch (error) {
        logger.error("Error processing message:", error);
        socket.emit("message", {
          type: "error",
          content: "Failed to process message",
        });
      }
    });

    // Handle disconnection
    socket.on("disconnect", () => {
      logger.info("Client disconnected");
    });
  });

  // Start WebSocket server
  httpServer.listen(WEBSOCKET_PORT, () => {
    logger.info(`WebSocket server listening on port ${WEBSOCKET_PORT}`);
  });

  return httpServer;
}

/**
 * Main entry point
 */
async function main() {
  try {
    logger.info("Starting agent...");
    logger.info(`Mode: ${config.AGENT_MODE}`);

    if (config.AGENT_MODE === "http") {
      await startServer();
    } else {
      await startWebSocketServer();
    }
  } catch (error) {
    logger.error("Fatal error:", error);
    process.exit(1);
  }
}

// Handle graceful shutdown
process.on("SIGINT", () => {
  logger.info("Shutting down...");
  process.exit(0);
});

process.on("SIGTERM", () => {
  logger.info("Shutting down...");
  process.exit(0);
});

// Start the application
if (require.main === module) {
  main().catch((error) => {
    logger.error("Fatal error:", error);
    process.exit(1);
  });
}

// Export for testing
export { startServer, startWebSocketServer };
