import { AgentKit } from "@coinbase/agentkit";
import { getLangChainTools } from "@coinbase/agentkit-langchain";
import { createReactAgent } from "@langchain/langgraph/prebuilt";
import { HumanMessage } from "@langchain/core/messages";
import { ChatOpenAI } from "@langchain/openai";
import * as dotenv from "dotenv";
import * as readline from "readline";
import { config } from "./config";
import {
  AgentResponse,
  AgentStatus,
  LangChainAgent,
  LangChainAgentConfig,
  InitializeResult,
} from "./types";

// Load environment variables
dotenv.config();

const AGENT_PROMPT = `
  You are a helpful agent that can assist users with various tasks.
  You are powered by the Coinbase Developer Platform and can help with:
  - Answering questions
  - Processing information
  - Providing assistance with tasks

  If someone asks you to do something you can't do with your currently available tools,
  you must say so and explain what capabilities you do have.

  Be concise and helpful with your responses.
  Refrain from restating your tools' descriptions unless explicitly requested.
`;

/**
 * Initialize the agent with the Coinbase Agent Kit
 * @returns A fully configured LangChain agent and config
 */
async function initialize(): Promise<InitializeResult> {
  // Initialize AgentKit with configuration
  const agentkit = await AgentKit.from({
    cdpApiKeyName: config.CDP_API_KEY_NAME,
    cdpApiKeyPrivateKey: config.CDP_API_KEY_PRIVATE_KEY,
    actionProviders: [], // Add your action providers here
  });

  // Get LangChain tools with AgentKit integration
  const tools = await getLangChainTools(agentkit);

  // Initialize LLM
  const llm = new ChatOpenAI({
    modelName: config.MODEL,
    temperature: 0,
    openAIApiKey: config.OPENAI_API_KEY,
  });

  // Create the agent configuration
  const agentConfig: LangChainAgentConfig = {
    configurable: {
      thread_id: "Coinbase Agent Kit Example",
    },
  };

  // Create the agent
  const agent = await createReactAgent({
    llm,
    tools,
    messageModifier: AGENT_PROMPT,
  });

  return { agent, config: agentConfig };
}

/**
 * Process the streaming response
 * @param chunk The chunk from the stream
 * @returns The processed content
 */
function processStreamChunk(chunk: any): string | null {
  if ("agent" in chunk && chunk.agent?.messages?.length > 0) {
    return chunk.agent.messages[0]?.content;
  } else if ("tools" in chunk && chunk.tools?.messages?.length > 0) {
    return chunk.tools.messages[0]?.content;
  }
  return null;
}

/**
 * Run the agent in chat mode
 * @param agent The LangChain agent instance
 * @param config Agent configuration
 */
export async function runChatMode(
  agent: LangChainAgent,
  config: LangChainAgentConfig
): Promise<void> {
  console.log("Starting chat mode... Type 'exit' to end.");

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  const question = (prompt: string): Promise<string> =>
    new Promise((resolve) => rl.question(prompt, resolve));

  try {
    while (true) {
      const userInput = await question("\nPrompt: ");

      if (userInput.toLowerCase() === "exit") {
        break;
      }

      try {
        // Create the user message
        const userMessage = new HumanMessage(userInput);

        // Stream the response, which is the proper way to interact with agents
        if (agent.stream) {
          const stream = await agent.stream(
            { messages: [userMessage] },
            config.configurable
          );

          // Process the streaming response
          for await (const chunk of stream) {
            const content = processStreamChunk(chunk);
            if (content) {
              console.log(content);
            }
          }
          console.log("-------------------");
        } else if (agent.invoke) {
          // Fallback to invoke if stream is not available
          const result = await agent.invoke(
            { messages: [userMessage] },
            config.configurable
          );
          const response = extractResponseContent(result);
          console.log("\nAgent:", response);
          console.log("-------------------");
        } else {
          throw new Error("Agent does not support stream or invoke methods");
        }
      } catch (processingError) {
        console.error(
          "Processing error:",
          processingError instanceof Error
            ? processingError.message
            : String(processingError)
        );
      }
    }
  } finally {
    rl.close();
  }
}

/**
 * Extract response content from various result formats
 * @param result The result from agent.invoke()
 * @returns A string representation of the response
 */
function extractResponseContent(result: unknown): string {
  if (typeof result === "string") {
    return result;
  } else if (result && typeof result === "object") {
    if ("content" in result && typeof result.content === "string") {
      return result.content;
    } else if ("response" in result && typeof result.response === "string") {
      return result.response;
    } else if (Array.isArray(result) && result.length > 0) {
      const lastMessage = result[result.length - 1];
      return typeof lastMessage === "string"
        ? lastMessage
        : lastMessage?.content || "No response content";
    } else if ("agent" in result && result.agent?.messages?.[0]?.content) {
      return result.agent.messages[0].content;
    }
  }

  return "The agent processed your request but returned an unrecognized format.";
}

/**
 * Agent class for HTTP server mode
 */
export class Agent {
  private _agent: LangChainAgent | null = null;
  private _agentConfig: LangChainAgentConfig | null = null;
  private readonly startTime: number;

  constructor() {
    this.startTime = Date.now();
  }

  /**
   * Initialize the agent
   */
  async initialize(): Promise<void> {
    try {
      const { agent, config } = await initialize();
      this._agent = agent;
      this._agentConfig = config;
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      console.error(`Failed to initialize agent: ${errorMsg}`);
      throw new Error(`Agent initialization failed: ${errorMsg}`);
    }
  }

  /**
   * Get the agent instance
   */
  get agent(): LangChainAgent | null {
    return this._agent;
  }

  /**
   * Get the agent configuration
   */
  get agentConfig(): LangChainAgentConfig | null {
    return this._agentConfig;
  }

  /**
   * Process a user message and return a response
   * @param message The user's message
   * @returns The agent's response with metadata
   * @throws Error if agent is not initialized or message processing fails
   */
  async processMessage(message: string): Promise<AgentResponse> {
    if (!this._agent || !this._agentConfig) {
      throw new Error("Agent not initialized");
    }

    try {
      const userMessage = new HumanMessage(message);
      let responseContent: string;

      // Use the appropriate method based on agent capabilities
      if (this._agent.stream) {
        // Collect all the content from the stream
        let content = "";
        const stream = await this._agent.stream(
          { messages: [userMessage] },
          this._agentConfig.configurable
        );

        for await (const chunk of stream) {
          const chunkContent = processStreamChunk(chunk);
          if (chunkContent && typeof chunkContent === "string") {
            content += chunkContent;
          }
        }

        responseContent = content || "No response generated";
      } else if (this._agent.invoke) {
        // Fall back to the invoke method if stream is not available
        const result = await this._agent.invoke(
          { messages: [userMessage] },
          this._agentConfig.configurable
        );
        responseContent = extractResponseContent(result);
      } else {
        throw new Error("Agent has no supported interaction methods");
      }

      return {
        response: responseContent,
        metadata: {
          agentConfig: this._agentConfig,
        },
      };
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      console.error(`Error processing message: ${errorMsg}`);
      throw new Error("Failed to process message");
    }
  }

  /**
   * Get the current status of the agent
   * @returns Agent status information
   */
  getStatus(): AgentStatus {
    return {
      status: "running",
      uptime: Math.floor((Date.now() - this.startTime) / 1000),
      mode: config.AGENT_MODE,
    };
  }
}
