# Coinbase Agent Kit - Tangle Blueprint

This repository contains a Tangle Network Blueprint service that enables the creation, deployment, and management of AI agents built with the [Coinbase Agent Kit](https://docs.cdp.coinbase.com). It provides a secure, containerized environment for running AI agents with seamless integration to blockchain and web services.

## 🚀 Overview

The Coinbase Agent Kit Blueprint allows you to:

- **Create AI agents** with different capabilities
- **Deploy agents** in Docker containers or Trusted Execution Environments (TEEs)
- **Manage API keys** securely for each agent
- **Interact** with deployed agents via CLI, HTTP API, or autonomous mode

## 🏗️ Architecture

The system consists of several key components:

1. **Tangle Blueprint Service**: A Rust-based service that exposes jobs and queries for agent management
2. **Agent Templates**: TypeScript templates for different agent types in the `templates/` directory
3. **Docker Deployment**: Infrastructure for containerizing and running agents
4. **TEE Integration**: Optional secure enclave deployment for sensitive agents

### Job Handlers

The service provides two main job handlers:

- `create_agent`: Generates agent files from templates based on configuration
- `deploy_agent`: Deploys the agent as a Docker container or TEE

## 🛠️ Customizing the Agent Launchpad

You can extend the Blueprint to support your own agent types by modifying the following components:

### 1. Create a New Agent Template

Add your agent template to the `templates/` directory. See [Templates README](templates/README.md) for detailed instructions.

### 2. Add Customization Logic

If your agent requires special customization, modify this logic in the TypeScript project. Add your agent's services and tests to ensure it works locally before updating the Dockerfile and docker-compose.yml file.

In the Typescript project, you can expose new functionality for your agent by adding new files to the `src/` directory. You can modify on-demand configuration through new environment variables, such as for prompts, models, and other configurations that the Rust environment can pass to the docker deployment.

### 3. Update Agent Docker Compose

In your Typescript project, update the `docker-compose.yml` file to add your agent's services and tests to ensure it works with the Docker deployment.

### 4. Update Deployment Logic

If your agent requires special deployment configuration, modify `src/deploy_agent.rs`:

```rust
fn deploy_docker_container(
    agent_dir: &Path,
    agent_id: &str,
    deployment_id: &str,
    config: &GadgetConfiguration,
) -> Result<Option<String>, String> {
    // Add specialized configuration for your agent type
    // ...
}
```

## 🔐 Security Considerations

When extending the Blueprint with your own agent types:

1. **API Key Management**: Follow the secure pattern for handling API keys in `.env` files
2. **TEE Integration**: Use TEEs for agents handling sensitive data or private keys
3. **Access Control**: Implement appropriate access controls for your agent APIs
4. **Dependency Security**: Regularly update dependencies in your templates

## 🧪 Testing Your Extension

See the [Testing Guide](docs/testing.md) for details on testing your custom agent integration, including:

1. Unit tests for your customization logic
2. Integration tests for the full agent lifecycle
3. End-to-end tests for deployment and interaction

## 📚 Documentation

- [Templates Guide](templates/starter/README.md) - How to create and customize agent templates

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
