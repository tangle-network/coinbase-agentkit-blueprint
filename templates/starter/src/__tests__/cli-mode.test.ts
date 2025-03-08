import { spawn, ChildProcess } from "child_process";
import path from "path";
import { once } from "events";

describe("CLI Chat Mode Tests", () => {
  let cliProcess: ChildProcess | null = null;

  // Helper function to spawn the CLI process
  const spawnCliProcess = () => {
    const processPath = path.resolve(__dirname, "../../src/index.ts");

    // Set environment variables
    const env = {
      ...process.env,
      AGENT_MODE: "cli-chat",
      NODE_ENV: "test",
      OPENAI_API_KEY: "mock-api-key-123",
      // Prevent actual API calls in test
      MOCK_AGENT: "true",
    };

    // Spawn the process
    return spawn("ts-node", [processPath], {
      env,
      // Pipe stdio for testing
      stdio: ["pipe", "pipe", "pipe"],
    });
  };

  afterEach(() => {
    // Cleanup after each test
    if (cliProcess && !cliProcess.killed) {
      cliProcess.kill();
      cliProcess = null;
    }
  });

  // Test process startup
  it("should start in CLI mode correctly", async () => {
    cliProcess = spawnCliProcess();

    // Wait for process output
    const [data] = await once(cliProcess.stdout!, "data");
    const output = data.toString();

    // Check for expected startup message
    expect(output).toContain("Starting agent");
    expect(output).toContain("Mode: cli-chat");
  }, 10000);

  // Test process handles user input
  it("should process user input correctly", async () => {
    cliProcess = spawnCliProcess();

    // Wait for process to initialize
    let dataPromise = once(cliProcess.stdout!, "data");
    let data = (await dataPromise)[0].toString();

    // Wait for prompt
    while (!data.includes("initialized successfully")) {
      dataPromise = once(cliProcess.stdout!, "data");
      data = (await dataPromise)[0].toString();
    }

    // Send a test message
    cliProcess.stdin!.write("Hello, agent!\n");

    // Wait for response
    dataPromise = once(cliProcess.stdout!, "data");
    data = (await dataPromise)[0].toString();

    // In test mode with MOCK_AGENT=true, we expect a mock response
    expect(data).toContain("Hello, agent!");
  }, 15000);

  // Test process handles exit command
  it("should handle exit command gracefully", async () => {
    cliProcess = spawnCliProcess();

    // Wait for process to initialize
    let dataPromise = once(cliProcess.stdout!, "data");
    let data = (await dataPromise)[0].toString();

    // Wait for prompt
    while (!data.includes("initialized successfully")) {
      dataPromise = once(cliProcess.stdout!, "data");
      data = (await dataPromise)[0].toString();
    }

    // Send exit command
    cliProcess.stdin!.write("exit\n");

    // Wait for process to exit
    const [exitCode] = await once(cliProcess, "exit");
    expect(exitCode).toBe(0);
  }, 10000);

  // Test error handling
  it("should handle processing errors", async () => {
    // Set an environment variable to trigger an error in the mock
    cliProcess = spawn(
      "ts-node",
      [path.resolve(__dirname, "../../src/index.ts")],
      {
        env: {
          ...process.env,
          AGENT_MODE: "cli-chat",
          NODE_ENV: "test",
          OPENAI_API_KEY: "mock-api-key-123",
          MOCK_AGENT: "true",
          MOCK_ERROR: "true", // Signal to mock to throw an error
        },
        stdio: ["pipe", "pipe", "pipe"],
      }
    );

    // Wait for process to initialize
    let dataPromise = once(cliProcess.stdout!, "data");
    let data = (await dataPromise)[0].toString();

    // Wait for prompt
    while (!data.includes("initialized successfully")) {
      dataPromise = once(cliProcess.stdout!, "data");
      data = (await dataPromise)[0].toString();
    }

    // Send a test message that will trigger an error
    cliProcess.stdin!.write("trigger_error\n");

    // Wait for error response
    dataPromise = once(cliProcess.stderr!, "data");
    data = (await dataPromise)[0].toString();

    // Check for error message
    expect(data).toContain("Error");
  }, 15000);
});
