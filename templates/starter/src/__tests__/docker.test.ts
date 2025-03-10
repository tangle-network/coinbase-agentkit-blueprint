import { execSync } from "child_process";
import fetch from "node-fetch";
import dotenv from "dotenv";
import * as fs from "fs";

// Load environment variables
dotenv.config();

// Skip tests if Docker is not available
const hasDocker = (): boolean => {
  try {
    execSync("docker --version", { stdio: "ignore" });
    return true;
  } catch (e) {
    return false;
  }
};

// Only run these tests if Docker is available
const itIfDocker = hasDocker() ? it : it.skip;

describe("Docker Container Tests", () => {
  const TEST_PORT = 4567;
  const IMAGE_NAME = "agent-test-image";
  const CONTAINER_NAME = "agent-test-container";
  const TEST_TIMEOUT = 60000; // Longer timeout for Docker operations

  // Utility function for HTTP requests
  const fetchWithTimeout = async (
    url: string,
    options: any = {},
    timeout = 5000
  ): Promise<any> => {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeout);

    try {
      console.log(`Fetching ${url}...`);
      const response = await fetch(url, {
        ...options,
        signal: controller.signal,
      });

      const text = await response.text();
      const data = text ? JSON.parse(text) : {};

      return {
        status: response.status,
        data,
      };
    } catch (error) {
      if ((error as Error).name === "AbortError") {
        throw new Error(`Request timed out after ${timeout}ms`);
      }
      console.error(`Error fetching ${url}:`, error);
      throw error;
    } finally {
      clearTimeout(timeoutId);
    }
  };

  // Build the Docker image before tests
  beforeAll(async () => {
    if (!hasDocker()) {
      console.warn("Docker not available, skipping Docker tests");
      return;
    }

    try {
      // Make sure the container and image are removed before starting
      execSync(`docker rm -f ${CONTAINER_NAME} || true`, { stdio: "ignore" });
      execSync(`docker rmi -f ${IMAGE_NAME} || true`, { stdio: "ignore" });

      // Build Docker image
      console.log("Building Docker image...");
      execSync(`docker build -t ${IMAGE_NAME} .`, { stdio: "inherit" });
    } catch (error) {
      console.error("Failed to build Docker image:", error);
      throw error;
    }
  }, TEST_TIMEOUT);

  // Clean up after all tests
  afterAll(() => {
    if (!hasDocker()) return;

    try {
      // Remove container if it exists
      execSync(`docker rm -f ${CONTAINER_NAME} || true`, { stdio: "ignore" });

      // Remove image
      execSync(`docker rmi -f ${IMAGE_NAME} || true`, { stdio: "ignore" });
    } catch (error) {
      console.error("Cleanup failed:", error);
    }
  });

  // Helper to start the container before each test
  beforeEach(async () => {
    if (!hasDocker()) return;

    try {
      // Get the actual API key from environment or .env file
      const API_KEY = process.env.OPENAI_API_KEY;

      if (!API_KEY || API_KEY === "your_openai_api_key_here") {
        console.warn("No valid OpenAI API key found, skipping Docker tests");
        return;
      }

      // Start container with environment variables passed directly
      console.log(`Starting Docker container on port ${TEST_PORT}...`);
      execSync(
        `docker run -d --name ${CONTAINER_NAME} \
        -p ${TEST_PORT}:${TEST_PORT} \
        -e PORT=${TEST_PORT} \
        -e AGENT_MODE=http \
        -e NODE_ENV=test \
        -e MODEL=gpt-4o-mini \
        -e LOG_LEVEL=info \
        -e OPENAI_API_KEY="${API_KEY}" \
        ${IMAGE_NAME}`,
        { stdio: "inherit" }
      );

      // Wait for container to start
      console.log("Waiting for container to start...");
      await new Promise((resolve) => setTimeout(resolve, 15000)); // Increased wait time

      // Print container logs for debugging
      console.log("Container logs:");
      execSync(`docker logs ${CONTAINER_NAME}`, { stdio: "inherit" });

      // Print container status
      console.log("Container status:");
      execSync(`docker ps -a --filter "name=${CONTAINER_NAME}"`, {
        stdio: "inherit",
      });

      // Check if the container is actually running
      const containerStatus = execSync(
        `docker inspect --format='{{.State.Status}}' ${CONTAINER_NAME}`,
        {
          encoding: "utf-8",
        }
      ).trim();

      console.log(`Container status from inspect: ${containerStatus}`);

      if (containerStatus !== "running") {
        throw new Error(`Container is not running, status: ${containerStatus}`);
      }
    } catch (error) {
      console.error("Failed to start container:", error);
      throw error;
    }
  }, TEST_TIMEOUT);

  // Remove container after each test
  afterEach(() => {
    if (!hasDocker()) return;

    try {
      // Print final logs
      console.log("Final container logs:");
      execSync(`docker logs ${CONTAINER_NAME} 2>&1 || true`, {
        stdio: "inherit",
      });

      // Stop and remove the container
      execSync(`docker stop ${CONTAINER_NAME} || true`, { stdio: "ignore" });
      execSync(`docker rm ${CONTAINER_NAME} || true`, { stdio: "ignore" });
    } catch (error) {
      console.error("Container cleanup failed:", error);
    }
  });

  // Test that container is running and health endpoint works
  itIfDocker(
    "should have working health endpoint",
    async () => {
      // Try the health check multiple times with a delay
      let success = false;
      let lastError: any = null;
      let response: any = null;

      for (let i = 0; i < 3; i++) {
        try {
          console.log(`Health check attempt ${i + 1}...`);
          response = await fetchWithTimeout(
            `http://localhost:${TEST_PORT}/health`,
            {},
            10000 // Increased timeout
          );
          success = true;
          break;
        } catch (err) {
          lastError = err;
          // Wait a bit before retrying
          await new Promise((resolve) => setTimeout(resolve, 5000));
        }
      }

      if (!success) {
        console.error("All health check attempts failed:", lastError);
        throw lastError;
      }

      expect(response.status).toBe(200);
      expect(response.data).toHaveProperty("status", "ok");
    },
    TEST_TIMEOUT
  );

  // Test that status endpoint works
  itIfDocker(
    "should have working status endpoint",
    async () => {
      const response = await fetchWithTimeout(
        `http://localhost:${TEST_PORT}/status`,
        {},
        10000 // Increased timeout
      );

      expect(response.status).toBe(200);
      expect(response.data).toHaveProperty("status", "running");
    },
    TEST_TIMEOUT
  );

  // Test that interaction endpoint works with real request
  itIfDocker(
    "should process messages through Docker container",
    async () => {
      const message = "What is 2+2?";

      const response = await fetchWithTimeout(
        `http://localhost:${TEST_PORT}/interact`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ message }),
        },
        20000 // Increased timeout for LLM processing
      );

      console.log("Docker container response:", response.data);

      expect(response.status).toBe(200);
      expect(response.data).toHaveProperty("response");
      expect(typeof response.data.response).toBe("string");
      expect(response.data.response.length).toBeGreaterThan(0);
    },
    TEST_TIMEOUT
  );
});
