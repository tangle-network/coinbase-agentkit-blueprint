import { config } from "../config";
import { setupTestEnv } from "./mockUtils";

describe("Config", () => {
  const { restoreEnv } = setupTestEnv();

  afterAll(() => {
    // Restore environment variables after tests
    restoreEnv();
  });

  it("should load configuration from environment variables", () => {
    expect(config.OPENAI_API_KEY).toBe("test-openai-api-key");
    expect(config.MODEL).toBe("gpt-4o-mini");
    expect(config.AGENT_MODE).toBe("http");
    expect(config.PORT).toBe(3000);
    expect(config.WEBSOCKET_PORT).toBe(3001);
  });

  it("should handle missing environment variables with defaults", () => {
    // Temporarily unset an environment variable
    const originalPort = process.env.PORT;
    delete process.env.PORT;

    // Re-import to trigger reload of config
    jest.resetModules();
    const { config: reloadedConfig } = require("../config");

    // Check that the default port is used
    expect(reloadedConfig.PORT).toBe(3000); // Default value

    // Restore for other tests
    process.env.PORT = originalPort;
  });
});
