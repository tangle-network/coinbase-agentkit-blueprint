#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🧪 Running real-world tests...${NC}"

# Ensure the .env file exists
if [ ! -f ".env" ]; then
  if [ -f ".env.example" ]; then
    echo -e "${YELLOW}⚠️ Creating .env from .env.example...${NC}"
    cp .env.example .env
    echo -e "${YELLOW}⚠️ Please update .env with your API keys and run again${NC}"
    exit 1
  else
    echo -e "${RED}❌ No .env or .env.example found${NC}"
    exit 1
  fi
fi

# Run the WebSocket tests
echo -e "${YELLOW}🔌 Running real WebSocket tests...${NC}"
yarn jest src/__tests__/real-websocket.test.ts --config=jest.plain.config.js --testTimeout=30000
WS_RESULT=$?

# Run Docker tests
echo -e "${YELLOW}🐳 Running Docker tests...${NC}"
yarn jest src/__tests__/docker.test.ts --config=jest.plain.config.js --testTimeout=60000
DOCKER_RESULT=$?

# Summary
echo ""
echo -e "${YELLOW}=== Real Test Results ===${NC}"
echo -e "WebSocket: $([ $WS_RESULT -eq 0 ] && echo "${GREEN}PASSED${NC}" || echo "${RED}FAILED${NC}")"
echo -e "Docker: $([ $DOCKER_RESULT -eq 0 ] && echo "${GREEN}PASSED${NC}" || echo "${RED}FAILED${NC}")"

# Exit with error if any test failed
if [ $WS_RESULT -ne 0 ] || [ $DOCKER_RESULT -ne 0 ]; then
  echo -e "${RED}❌ Some real-world tests failed${NC}"
  exit 1
else
  echo -e "${GREEN}✅ All real-world tests passed!${NC}"
  exit 0
fi 