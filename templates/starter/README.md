# Coinbase Agent Kit - Starter Template

This is a starter template for building agents with the Coinbase Agent Kit. It provides a production-ready foundation with TypeScript support, proper error handling, logging, and both HTTP and CLI interfaces.

## Prerequisites

- Node.js >= 18.0.0
- npm >= 7.0.0
- Docker (optional)

## Quick Start

1. Copy the `.env.example` file to `.env`:

   ```bash
   cp .env.example .env
   ```

2. Edit the `.env` file with your configuration:

   - Add your OpenAI API key
   - Configure CDP API keys if needed
   - Adjust other settings as needed

3. Install dependencies:

   ```bash
   yarn install
   ```

4. Start the development server:
   ```bash
   yarn dev
   ```

## Available Scripts

- `npm run build` - Build the TypeScript code
- `npm start` - Start the production server
- `npm run dev` - Start the development server with hot reload
- `npm test` - Run all tests
- `./run-all-tests.sh` - Run comprehensive test suite including all integration tests
- `npm run lint` - Run ESLint
- `npm run format` - Format code with Prettier
- `npm run docker:build` - Build Docker image
- `npm run docker:run` - Run Docker container

## Docker Support

Build the Docker image:

```bash
npm run docker:build
```

Run the container:

```bash
npm run docker:run
```

Or manually:

```bash
docker run -p 3000:3000 --env-file .env coinbase-agent-kit-starter
```

## API Endpoints

When running in HTTP mode:

- `GET /status` - Get agent status
- `POST /interact` - Send a message to the agent
  ```json
  {
    "message": "Your message here"
  }
  ```

## CLI Mode

To run in CLI mode, set `AGENT_MODE=cli-chat` in your `.env` file. The agent will start an interactive chat session.

## Environment Variables

See `.env.example` for all available configuration options.

Required:

- `OPENAI_API_KEY` - Your OpenAI API key

Optional:

- `PORT` - HTTP server port (default: 3000)
- `AGENT_MODE` - `http` or `cli-chat` (default: http)
- `MODEL` - OpenAI model to use (default: gpt-4o-mini)
- `CDP_API_KEY_NAME` - CDP API key name
- `CDP_API_KEY_PRIVATE_KEY` - CDP API key private key

## Project Structure

```
.
├── src/
│   ├── index.ts        # Application entry point
│   ├── agent.ts        # Agent implementation
│   ├── config.ts       # Configuration management
│   ├── logger.ts       # Logging setup
│   ├── types.ts        # TypeScript types
│   └── __tests__/      # Test files
├── Dockerfile          # Docker configuration
├── docker-test.sh      # Docker integration tests
├── run-all-tests.sh    # Comprehensive test runner
├── run-integration-tests.sh # HTTP integration test runner
├── package.json        # Dependencies and scripts
└── tsconfig.json       # TypeScript configuration
```

## License

MIT

### Running All Tests

The `run-all-tests.sh` script executes all test categories and provides a detailed summary:

```bash
./run-all-tests.sh
```

### Test Configuration

- Tests can be run with real API calls by setting `RUN_API_TESTS=true`
- Mocked tests run faster and don't require API keys
- Docker tests verify deployment behavior
- WebSocket tests ensure real-time communication works

### Continuous Integration

The testing framework is designed to work in CI/CD environments. Just make sure to:

1. Set the appropriate environment variables
2. Run the comprehensive test suite with `./run-all-tests.sh`
3. Include Docker tests if container deployment is part of your workflow
