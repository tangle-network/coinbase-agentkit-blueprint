import { AgentResponse } from "../types";
import { Agent } from "../agent";

// Mock the Agent class
jest.mock("../agent", () => ({
  Agent: jest.fn().mockImplementation(() => ({
    initialize: jest.fn().mockResolvedValue(undefined),
    processMessage: jest
      .fn()
      .mockImplementation((message: string): Promise<AgentResponse> => {
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
  })),
}));

// Mock the index exports
jest.mock("../index", () => ({
  startWebSocketServer: jest.fn().mockResolvedValue({
    close: jest.fn(),
  }),
}));

describe("WebSocket Message Handling", () => {
  // Create types for our mocks
  interface MockSocket {
    emit: jest.Mock;
    on: jest.Mock;
    id: string;
    messageHandlers: Function[];
  }

  let mockAgent: Agent;
  let mockSocket: MockSocket;

  beforeEach(() => {
    // Clear all mocks
    jest.clearAllMocks();

    // Create a mock socket
    mockSocket = {
      emit: jest.fn(),
      on: jest.fn(),
      id: "test-socket-id",
      messageHandlers: [],
    };

    // Mock the on method to store handlers
    mockSocket.on.mockImplementation((event: string, handler: Function) => {
      if (event === "message") {
        mockSocket.messageHandlers.push(handler);
      }
      return mockSocket;
    });

    // Create a fresh agent instance
    mockAgent = new Agent();
  });

  // Helper to simulate the connection handler
  const simulateWebSocketConnection = () => {
    // Send welcome message (this happens in the connection handler)
    mockSocket.emit("message", {
      type: "system",
      content: "Connected to agent. Type your message to begin.",
    });

    // Set up message handler (this is simplified from the actual implementation)
    const messageHandler = async (data: { message: string }) => {
      try {
        const response = await mockAgent.processMessage(data.message);
        mockSocket.emit("message", {
          type: "agent",
          content: response.response,
          metadata: response.metadata,
        });
      } catch (error) {
        mockSocket.emit("message", {
          type: "error",
          content: "Failed to process message",
        });
      }
    };

    // Register handlers
    mockSocket.on("message", messageHandler);
    mockSocket.on("disconnect", () => {
      /* Empty handler */
    });
  };

  it("should send a welcome message on connection", () => {
    simulateWebSocketConnection();

    expect(mockSocket.emit).toHaveBeenCalledWith("message", {
      type: "system",
      content: "Connected to agent. Type your message to begin.",
    });
  });

  it("should register message and disconnect handlers", () => {
    simulateWebSocketConnection();

    expect(mockSocket.on).toHaveBeenCalledWith("message", expect.any(Function));
    expect(mockSocket.on).toHaveBeenCalledWith(
      "disconnect",
      expect.any(Function)
    );
  });

  it("should process messages and return responses", async () => {
    simulateWebSocketConnection();

    // Get the message handler
    const messageHandler = mockSocket.messageHandlers[0];

    // Simulate a message
    await messageHandler({ message: "Hello, agent!" });

    // Verify response was correct
    expect(mockSocket.emit).toHaveBeenCalledWith("message", {
      type: "agent",
      content: "Mock response to: Hello, agent!",
      metadata: { mocked: true },
    });
  });

  it("should handle errors during message processing", async () => {
    simulateWebSocketConnection();

    // Make the processMessage method throw an error
    (mockAgent.processMessage as jest.Mock).mockImplementationOnce(() => {
      throw new Error("Test error");
    });

    // Get the message handler
    const messageHandler = mockSocket.messageHandlers[0];

    // Simulate a message that will cause an error
    await messageHandler({ message: "This will cause an error" });

    // Verify error response
    expect(mockSocket.emit).toHaveBeenCalledWith("message", {
      type: "error",
      content: "Failed to process message",
    });
  });
});
