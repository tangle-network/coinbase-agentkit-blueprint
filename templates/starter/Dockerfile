# Single-stage build for simplicity
FROM node:18-slim

# Create app directory
WORKDIR /app

# Enable Corepack to use the project-specified Yarn version
RUN corepack enable

# Copy package files
COPY package.json yarn.lock ./

# Install dependencies with frozen lockfile for reproducibility
RUN corepack prepare yarn@stable --activate && \
    yarn install --frozen-lockfile && \
    yarn cache clean

# Copy application code 
COPY . .

# Build the application if needed
RUN if grep -q "\"build\":" package.json; then yarn build; fi

# Default values which can be overridden at runtime
ENV PORT=3000
ENV WEBSOCKET_PORT=3001
ENV AGENT_MODE=http
ENV NODE_ENV=production
ENV MODEL=gpt-4o-mini
ENV LOG_LEVEL=info
ENV DOCKER_IMAGE=true
# Secret keys for API
ENV OPENAI_API_KEY=""
ENV CDP_API_KEY_NAME=""
ENV CDP_API_KEY_PRIVATE_KEY=""


# Expose all potentially used ports
EXPOSE 3000
EXPOSE 3001
EXPOSE 4567

# Use a more efficient startup
ENTRYPOINT ["sh", "-c", "yarn start"]