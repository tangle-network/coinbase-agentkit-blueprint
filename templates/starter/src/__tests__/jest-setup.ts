// Import Jest types
import "@jest/globals";

// This file gets executed before tests run
// Set up any global configuration, mocks, or fixes here

// Make console.error less noisy during tests
const originalConsoleError = console.error;
console.error = (...args) => {
  // Don't log expected errors during tests
  if (
    typeof args[0] === "string" &&
    (args[0].includes("Error processing message") ||
      args[0].includes("Test error"))
  ) {
    return;
  }
  originalConsoleError(...args);
};
