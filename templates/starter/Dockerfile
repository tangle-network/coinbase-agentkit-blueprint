# Use Node.js LTS as the base image
FROM node:18-slim

# Create app directory
WORKDIR /app

# Install dependencies first (better layer caching)
COPY package*.json ./
RUN npm install

# Copy source code
COPY . .

# Expose ports
EXPOSE 3000
EXPOSE 3001

# Use ts-node to run TypeScript directly
CMD ["npm", "run", "start"]