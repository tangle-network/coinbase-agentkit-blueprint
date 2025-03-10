# Single-stage build for simplicity
FROM node:18-slim

# Create app directory
WORKDIR /app

# Enable Corepack to use the project-specified Yarn version
RUN corepack enable

# Copy package files
COPY package.json yarn.lock ./

# Install dependencies
RUN yarn install

# Copy application code
COPY . .

# Default values which can be overridden at runtime
ENV PORT=3000
ENV WEBSOCKET_PORT=3001
ENV AGENT_MODE=http
ENV NODE_ENV=production
ENV MODEL=gpt-4o-mini
ENV LOG_LEVEL=info

# Expose all potentially used ports
EXPOSE 3000
EXPOSE 3001
EXPOSE 4567

# Use bash to run commands so environment variables are properly expanded
ENTRYPOINT ["yarn", "start"]