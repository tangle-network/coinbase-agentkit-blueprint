import { AgentKit } from "@coinbase/agentkit";
import { getLangChainTools } from "@coinbase/agentkit-langchain";
import { HumanMessage } from "@langchain/core/messages";
import { MemorySaver } from "@langchain/langgraph";
import { createReactAgent } from "@langchain/langgraph/prebuilt";
import { ChatOpenAI } from "@langchain/openai";
import * as dotenv from "dotenv";
import * as readline from "readline";
import { config } from "./config";

// Load environment variables
dotenv.config();

const modifier = `
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
 */
async function initialize() {
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
  const agentConfig = {
    configurable: {
      thread_id: "Coinbase Agent Kit Example",
    },
  };

  // Create the agent
  const agent = await createReactAgent({
    llm,
    tools,
    messageModifier: modifier,
  });

  return { agent, config: agentConfig };
}

/**
 * Run the agent in chat mode
 */
export async function runChatMode(agent: any, agentConfig: any) {
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

      const response = await agent.chat(userInput);
      console.log("\nAgent:", response);
      console.log("-------------------");
    }
  } catch (error) {
    if (error instanceof Error) {
      console.error("Error:", error.message);
    }
    process.exit(1);
  } finally {
    rl.close();
  }
}

/**
 * Run the agent in HTTP server mode
 */
export class Agent {
  private _agent: any;
  private _agentConfig: any;
  private startTime: number;

  constructor() {
    this.startTime = Date.now();
  }

  async initialize() {
    const { agent, config } = await initialize();
    this._agent = agent;
    this._agentConfig = config;
  }

  /**
   * Get the agent instance
   */
  get agent() {
    return this._agent;
  }

  /**
   * Get the agent configuration
   */
  get agentConfig() {
    return this._agentConfig;
  }

  /**
   * Process a user message and return a response
   */
  async processMessage(message: string) {
    try {
      const response = await this._agent.chat(message);

      return {
        response,
        metadata: {
          agentConfig: this._agentConfig,
        },
      };
    } catch (error) {
      console.error("Error processing message:", error);
      throw new Error("Failed to process message");
    }
  }

  /**
   * Get the current status of the agent
   */
  getStatus() {
    return {
      status: "running",
      uptime: Math.floor((Date.now() - this.startTime) / 1000),
      mode: config.AGENT_MODE,
    };
  }
}
