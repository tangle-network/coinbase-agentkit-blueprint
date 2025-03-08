module.exports = {
  preset: "ts-jest",
  testEnvironment: "node",
  transform: {
    "^.+\\.tsx?$": [
      "ts-jest",
      { isolatedModules: true, noEmit: true, allowJs: true },
    ],
  },
  moduleFileExtensions: ["ts", "tsx", "js", "jsx", "json", "node"],
  testTimeout: 30000,
  collectCoverage: false,
  // Skip coverage reports to simplify
  setupFiles: ["dotenv/config"],
};
