# Build stage
FROM node:18-slim as build

# Create app directory
WORKDIR /app

# Enable Corepack for Yarn 4 support
RUN corepack enable

# Install dependencies first (better layer caching)
COPY package*.json ./
COPY yarn.lock ./
COPY .yarn ./.yarn
RUN yarn install

# Copy source code
COPY . .

# Run tests during build (optional, controlled by build args)
ARG RUN_TESTS=false
RUN if [ "$RUN_TESTS" = "true" ]; then \
      echo "Running tests..." && \
      yarn test; \
    else \
      echo "Skipping tests..."; \
    fi

# Production stage
FROM node:18-slim

# Enable Corepack for Yarn 4 support
RUN corepack enable

# Create app directory
WORKDIR /app

# Copy from build stage
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/package.json ./
COPY --from=build /app/tsconfig.json ./
COPY --from=build /app/src ./src
COPY --from=build /app/.yarn ./.yarn

# Expose ports
EXPOSE 3000
EXPOSE 3001

# Use ts-node to run TypeScript directly
CMD ["npm", "run", "start"]