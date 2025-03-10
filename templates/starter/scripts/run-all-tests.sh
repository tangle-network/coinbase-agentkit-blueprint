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
)

# Real test configurations with longer timeouts
real_test_groups=(
  "Real WebSocket:src/__tests__/real-websocket.test.ts:30000"
  "Docker:src/__tests__/docker.test.ts:60000"
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

# Run real test groups with their specified timeouts
echo -e "\n${YELLOW}🧪 Running real-world tests...${NC}"

for real_test in "${real_test_groups[@]}"; do
  # Split group name, files, and timeout
  IFS=':' read -r name files timeout <<< "$real_test"
  
  echo -e "\n${BLUE}=== Testing $name ===${NC}"
  
  if yarn jest $files --config=jest.plain.config.js --testTimeout=$timeout; then
    results+=("${GREEN}✅ $name: PASSED${NC}")
  else
    results+=("${RED}❌ $name: FAILED${NC}")
    overall_status=1
  fi
done

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