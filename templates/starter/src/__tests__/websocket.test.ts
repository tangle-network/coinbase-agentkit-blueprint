import { Server } from "http";
import { io as Client, Socket as ClientSocket } from "socket.io-client";
import { startWebSocketServer } from "../index";
import { Agent } from "../agent";

// Mock the Agent class
jest.mock("../agent", () => {
  return {
    Agent: jest.fn().mockImplementation(() => {
      return {
        initialize: jest.fn().mockResolvedValue(undefined),
        processMessage: jest.fn().mockImplementation((message: string) => {
          return Promise.resolve({
            response: `Mock response to: ${message}`,
            metadata: { mocked: true },
          });
        }),
        getStatus: jest.fn().mockReturnValue({
          status: "running",
          uptime: 60,
          mode: "websocket",
        }),
      };
    }),
  };
});

describe("WebSocket Server Tests", () => {
  let server: Server;
  let clientSocket: ClientSocket;
  const TEST_PORT = 3099; // Use a unique port for testing

  beforeAll((done) => {
    // Override the port for testing
    process.env.WEBSOCKET_PORT = TEST_PORT.toString();

    // Start the WebSocket server
    startWebSocketServer().then((httpServer) => {
      server = httpServer;

      // Wait a bit for the server to be ready
      setTimeout(() => {
        // Connect a client
        clientSocket = Client(`http://localhost:${TEST_PORT}`, {
          transports: ["websocket"],
          forceNew: true,
        });

        clientSocket.on("connect", done);
      }, 300);
    });
  });

  afterAll((done) => {
    // Cleanup
    if (clientSocket) {
      clientSocket.disconnect();
    }

    if (server) {
      server.close(() => {
        done();
      });
    } else {
      done();
    }
  });

  // Basic connection test
  it("should connect successfully", () => {
    expect(clientSocket.connected).toBe(true);
  });

  // Test welcome message
  it("should receive a welcome message on connection", (done) => {
    clientSocket.once("message", (data) => {
      expect(data).toHaveProperty("type", "system");
      expect(data).toHaveProperty("content");
      expect(data.content).toContain("Connected to agent");
      done();
    });
  });

  // Test sending a message and receiving a response
  it("should process messages and return responses", (done) => {
    const testMessage = "Hello, agent!";

    clientSocket.once("message", (data) => {
      expect(data).toHaveProperty("type", "agent");
      expect(data).toHaveProperty("content");
      expect(data.content).toBe(`Mock response to: ${testMessage}`);
      expect(data).toHaveProperty("metadata");
      expect(data.metadata).toHaveProperty("mocked", true);
      done();
    });

    clientSocket.emit("message", { message: testMessage });
  });

  // Test error handling - invalid message format
  it("should handle message processing errors gracefully", (done) => {
    // Override the mock to throw an error
    const mockAgent = Agent as jest.Mock;
    const mockInstance = mockAgent.mock.results[0].value;
    const originalProcessMessage = mockInstance.processMessage;

    mockInstance.processMessage = jest.fn().mockImplementation(() => {
      throw new Error("Test error");
    });

    clientSocket.once("message", (data) => {
      expect(data).toHaveProperty("type", "error");
      expect(data.content).toBe("Failed to process message");

      // Restore the original mock
      mockInstance.processMessage = originalProcessMessage;
      done();
    });

    clientSocket.emit("message", { message: "This will cause an error" });
  });

  // Test continuous conversation
  it("should handle a continuous conversation", (done) => {
    const messages = ["First message", "Second message", "Third message"];
    let messageIndex = 0;

    function sendNextMessage() {
      if (messageIndex < messages.length) {
        const currentMessage = messages[messageIndex];

        clientSocket.once("message", (data) => {
          expect(data).toHaveProperty("type", "agent");
          expect(data.content).toBe(`Mock response to: ${currentMessage}`);

          messageIndex++;
          sendNextMessage();
        });

        clientSocket.emit("message", { message: currentMessage });
      } else {
        done();
      }
    }

    sendNextMessage();
  });
});
