module.exports = {
  preset: "ts-jest",
  testEnvironment: "node",
  testMatch: ["**/__tests__/**/*.ts", "**/*.test.ts", "**/*.spec.ts"],
  transform: {
    "^.+\\.tsx?$": "ts-jest",
  },
  moduleFileExtensions: ["ts", "tsx", "js", "jsx", "json", "node"],
  collectCoverage: true,
  coverageDirectory: "coverage",
  collectCoverageFrom: [
    "src/**/*.{ts,tsx}",
    "!src/**/*.d.ts",
    "!src/**/*.test.ts",
    "!src/**/*.spec.ts",
  ],
  coverageThreshold: {
    global: {
      branches: 0,
      functions: 0,
      lines: 0,
      statements: 0,
    },
  },
  setupFiles: ["dotenv/config"],
  setupFilesAfterEnv: ["<rootDir>/src/__tests__/jest-setup.ts"],
  testTimeout: 30000,

  // Handle different test types with specific configurations
  projects: [
    {
      // Unit tests configuration
      displayName: "unit",
      testMatch: ["<rootDir>/src/**/*.test.ts"],
      testPathIgnorePatterns: [
        "integration.test.ts",
        "websocket.test.ts",
        "cli-mode.test.ts",
        "config.integration.test.ts",
      ],
    },
    {
      // Server integration tests
      displayName: "server-integration",
      testMatch: ["<rootDir>/src/__tests__/agent-system.test.ts"],
      testTimeout: 30000,
    },
    {
      // WebSocket tests
      displayName: "websocket",
      testMatch: ["<rootDir>/src/__tests__/websocket.test.ts"],
      testTimeout: 30000,
    },
    {
      // CLI mode tests
      displayName: "cli-mode",
      testMatch: ["<rootDir>/src/__tests__/cli-mode.test.ts"],
      testTimeout: 30000,
    },
    {
      // Configuration tests
      displayName: "config",
      testMatch: ["<rootDir>/src/__tests__/config*.test.ts"],
      testTimeout: 30000,
    },
  ],
};
