#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configurations
test_groups=(
  "HTTP:src/__tests__/agent-system.test.ts src/__tests__/server.test.ts"
  "WebSocket:src/__tests__/websocket.test.ts"
  "Config:src/__tests__/config.integration.test.ts"
  "CLI:src/__tests__/cli-mode.test.ts"
)

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
  echo -e "${YELLOW}📦 Installing dependencies...${NC}"
  yarn install
fi

# Create a test .env file if it doesn't exist
if [ ! -f ".env" ] && [ -f ".env.example" ]; then
  echo -e "${YELLOW}Creating test .env file from .env.example...${NC}"
  cp .env.example .env
  echo -e "${YELLOW}⚠️ Created .env from example. Please edit it to add your API key.${NC}"
  echo -e "${YELLOW}Then run this script again.${NC}"
  exit 1
fi

# Set environment
export NODE_ENV=test
export TS_NODE_TRANSPILE_ONLY=1
export RUN_API_TESTS=true

# Test results
results=()
overall_status=0

# Run test groups
echo -e "${YELLOW}🧪 Running test suite...${NC}"

for test_group in "${test_groups[@]}"; do
  # Split group name and files
  IFS=':' read -r name files <<< "$test_group"
  
  echo -e "\n${BLUE}=== Testing $name ===${NC}"
  
  if yarn jest $files --config=jest.plain.config.js --testTimeout=30000; then
    results+=("${GREEN}✅ $name: PASSED${NC}")
  else
    results+=("${RED}❌ $name: FAILED${NC}")
    overall_status=1
  fi
done

# Run Docker tests if Docker is available
if command -v docker &> /dev/null && docker info &> /dev/null; then
  echo -e "\n${BLUE}=== Testing Docker ===${NC}"
  if ./scripts/docker-test.sh; then
    results+=("${GREEN}✅ Docker: PASSED${NC}")
  else
    results+=("${RED}❌ Docker: FAILED${NC}")
    overall_status=1
  fi
else
  results+=("${YELLOW}⚠️ Docker: SKIPPED (Docker not available)${NC}")
fi

# Summary
echo -e "\n${BLUE}=== Test Summary ===${NC}"
for result in "${results[@]}"; do
  echo -e "$result"
done

if [ $overall_status -eq 0 ]; then
  echo -e "\n${GREEN}✅ All tests passed!${NC}"
  exit 0
else
  echo -e "\n${RED}❌ Some tests failed.${NC}"
  exit 1
fi 