#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🐳 Testing Docker deployment...${NC}"

# Check if Docker is installed
if ! command -v docker &> /dev/null || ! command -v docker-compose &> /dev/null; then
    echo -e "${RED}❌ Docker or docker-compose not found. Please install Docker to run this test.${NC}"
    exit 1
fi

# Cleanup function
cleanup() {
    echo -e "${YELLOW}🧹 Cleaning up test containers...${NC}"
    docker-compose down
    if [ -f ".env.docker-test.bak" ]; then
        mv .env.docker-test.bak .env
    fi
}

# Register cleanup function
trap cleanup EXIT

# Back up current .env if it exists
if [ -f ".env" ]; then
    cp .env .env.docker-test.bak
fi

# Create test environment
echo -e "${YELLOW}📝 Setting up test environment...${NC}"
cat > .env << EOF
OPENAI_API_KEY=mock-api-key-for-testing
PORT=3456
WEBSOCKET_PORT=3457
AGENT_MODE=http
MODEL=gpt-4o-mini
CONTAINER_NAME=agent-test-container
MOCK_AGENT=true
NODE_ENV=test
RUN_TESTS=false
EOF

# Start the containers
echo -e "${YELLOW}🚀 Starting Docker containers...${NC}"
docker-compose up -d --build

# Wait for the service to start
echo -e "${YELLOW}⏳ Waiting for service to start...${NC}"
sleep 10

# Test HTTP health endpoint
echo -e "${YELLOW}🔍 Testing HTTP health endpoint...${NC}"
if curl -s -o /dev/null -w "%{http_code}" http://localhost:3456/health | grep -q "200"; then
    echo -e "${GREEN}✅ HTTP health check passed${NC}"
else
    echo -e "${RED}❌ HTTP health check failed${NC}"
    exit 1
fi

# Test HTTP status endpoint
echo -e "${YELLOW}🔍 Testing HTTP status endpoint...${NC}"
STATUS_RESPONSE=$(curl -s http://localhost:3456/status)
if echo "$STATUS_RESPONSE" | grep -q "running"; then
    echo -e "${GREEN}✅ HTTP status check passed: $STATUS_RESPONSE${NC}"
else
    echo -e "${RED}❌ HTTP status check failed: $STATUS_RESPONSE${NC}"
    exit 1
fi

# Test HTTP interaction endpoint
echo -e "${YELLOW}🔍 Testing HTTP interaction endpoint...${NC}"
INTERACTION_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
                     -d '{"message":"Hello from Docker test!"}' \
                     http://localhost:3456/interact)
if echo "$INTERACTION_RESPONSE" | grep -q "response"; then
    echo -e "${GREEN}✅ HTTP interaction test passed: $INTERACTION_RESPONSE${NC}"
else
    echo -e "${RED}❌ HTTP interaction test failed: $INTERACTION_RESPONSE${NC}"
    exit 1
fi

# All tests passed
echo -e "${GREEN}✅ All Docker tests passed!${NC}"
exit 0 