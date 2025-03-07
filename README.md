# Coinbase Agent Kit - Tangle Blueprint

This repository contains a Tangle Network Blueprint service that enables the creation, deployment, and management of AI agents built with the [Coinbase Agent Kit](https://docs.cdp.coinbase.com). It provides a secure, containerized environment for running AI agents with seamless integration to blockchain and web services.

## 🚀 Overview

The Coinbase Agent Kit Blueprint allows you to:

- **Create AI agents** with different capabilities (Twitter, Smart Wallet, Custom)
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

### Query Endpoints

The service also exposes query endpoints:

- `agent_status`: Check the status of a deployed agent
- `agent_interaction`: Send messages to an agent and get responses

## 🛠️ Customizing the Agent Launchpad

You can extend the Blueprint to support your own agent types by modifying the following components:

### 1. Create a New Agent Template

Add your agent template to the `templates/` directory. See [Templates README](templates/README.md) for detailed instructions.

### 2. Update Agent Types

Modify `src/types.rs` to add your new agent type:

```rust
pub enum AgentType {
    Twitter,
    SmartWallet,
    Custom,
    Starter,
    YourNewAgentType,  // Add your agent type here
}
```

### 3. Update Template Selection

Modify `src/create_agent.rs` to handle your new agent type:

```rust
fn copy_template_files(params: &CreateAgentParams, agent_id: &str, agent_dir: &Path) -> Result<Vec<String>, String> {
    // ...
    let template_dir = match params.agent_config.agent_type {
        AgentType::Twitter => "templates/twitter",
        AgentType::SmartWallet => "templates/smart_wallet",
        AgentType::Custom => "templates/custom",
        AgentType::Starter => "templates/starter",
        AgentType::YourNewAgentType => "templates/your_agent_directory",  // Add your template path
    };
    // ...
}
```

### 4. Add Customization Logic (Optional)

If your agent requires special customization, add a handler in `customize_agent_files`:

```rust
fn customize_agent_files(params: &CreateAgentParams, agent_dir: &Path) -> Result<(), String> {
    // ...
    match params.agent_config.agent_type {
        AgentType::Custom => customize_custom_agent(params, agent_dir)?,
        AgentType::YourNewAgentType => customize_your_agent(params, agent_dir)?,  // Add your customization
        _ => {}
    }
    // ...
}
```

### 5. Update Deployment Logic (Optional)

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

- [Templates Guide](templates/README.md) - How to create and customize agent templates
- [Quick Start Tutorial](templates/TUTORIAL.md) - Step-by-step guide for creating agents
- [API Reference](docs/api.md) - Blueprint service API documentation
- [Deployment Guide](docs/deployment.md) - How to deploy the service and agents

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-agent-type`)
3. Commit your changes (`git commit -am 'Add support for my agent type'`)
4. Push to the branch (`git push origin feature/my-agent-type`)
5. Create a new Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
