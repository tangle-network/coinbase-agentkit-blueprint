use dockworker::config::compose::{BuildConfig, ComposeConfig, Service};
use dockworker::config::EnvironmentVars;
use phala_tee_deploy_rs::{TeeDeployer, TeeDeployerBuilder};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Creates a Docker Compose configuration for a Coinbase Agent
///
/// This function generates a standardized Docker Compose configuration
/// that can be used for both local and TEE deployments.
///
/// # Arguments
///
/// * `agent_id` - Unique identifier for the agent
/// * `http_port` - The HTTP port to expose (default: 3000)
/// * `websocket_port` - The WebSocket port to expose (default: 3001)
/// * `env_vars` - Additional environment variables to include
///
/// # Returns
///
/// A tuple containing:
/// * The ComposeConfig object
/// * YAML string representation of the configuration
pub fn create_agent_compose_config(
    agent_id: &str,
    http_port: Option<u16>,
    websocket_port: Option<u16>,
    env_vars: HashMap<String, String>,
) -> (ComposeConfig, String) {
    let http_port = http_port.unwrap_or(3000);
    let websocket_port = websocket_port.unwrap_or(3001);

    // Create a Docker Compose configuration for the agent
    let mut compose_config = ComposeConfig::default();
    // Version is already set to "3" by default in ComposeConfig

    // Create the agent service
    let mut agent_service = Service::default();

    // Use build context instead of image
    agent_service.build = Some(BuildConfig {
        context: ".".to_string(),
        dockerfile: None, // Use default Dockerfile in the context directory
    });

    // Map ports for HTTP and WebSocket
    agent_service.ports = Some(vec![
        format!("{}:{}", http_port, http_port),
        format!("{}:{}", websocket_port, websocket_port),
    ]);

    // Set up environment variables
    let mut service_env = HashMap::new();
    service_env.insert("PORT".to_string(), http_port.to_string());
    service_env.insert("WEBSOCKET_PORT".to_string(), websocket_port.to_string());
    service_env.insert(
        "CONTAINER_NAME".to_string(),
        format!("coinbase-agent-{}", agent_id),
    );
    service_env.insert("NODE_ENV".to_string(), "production".to_string());
    service_env.insert("AGENT_MODE".to_string(), "http".to_string());
    service_env.insert("MODEL".to_string(), "gpt-4o-mini".to_string());
    service_env.insert("LOG_LEVEL".to_string(), "info".to_string());

    // Add additional environment variables
    for (key, value) in env_vars {
        service_env.insert(key, value);
    }

    // Set the environment variables in the service
    agent_service.environment = Some(EnvironmentVars::from(service_env));

    // Use restart policy for reliability
    agent_service.restart = Some("unless-stopped".to_string());

    // Add the agent service to the config
    compose_config
        .services
        .insert("agent".to_string(), agent_service);

    // Generate YAML from the config - we can't use serde_yaml directly since it's not a dependency
    // So we'll construct a basic YAML string manually
    let yaml = create_yaml_from_config(&compose_config, agent_id, http_port, websocket_port);

    (compose_config, yaml)
}

/// Helper function to manually create a YAML string from the compose config
fn create_yaml_from_config(
    _config: &ComposeConfig,
    agent_id: &str,
    http_port: u16,
    websocket_port: u16,
) -> String {
    // Create a basic YAML string manually since we can't use serde_yaml
    format!(
        r#"version: '3'
services:
  agent:
    container_name: coinbase-agent-{}
    build:
      context: .
    ports:
      - "{}:{}"
      - "{}:{}"
    environment:
      - PORT={}
      - WEBSOCKET_PORT={}
      - CONTAINER_NAME=coinbase-agent-{}
      - NODE_ENV=production
      - AGENT_MODE=http
      - MODEL=gpt-4o-mini
      - LOG_LEVEL=debug
      - OPENAI_API_KEY=${{OPENAI_API_KEY}}
      - CDP_API_KEY_NAME=${{CDP_API_KEY_NAME}}
      - CDP_API_KEY_PRIVATE_KEY=${{CDP_API_KEY_PRIVATE_KEY}}
    restart: unless-stopped
"#,
        agent_id,
        http_port,
        http_port,
        websocket_port,
        websocket_port,
        http_port,
        websocket_port,
        agent_id
    )
}

/// Creates a Docker Compose file in the agent directory
///
/// This function generates and writes a Docker Compose file that matches
/// the configuration used for TEE deployment.
///
/// # Arguments
///
/// * `agent_dir` - Path to the agent directory
/// * `agent_id` - Unique identifier for the agent
/// * `http_port` - The HTTP port to expose (default: 3000)
/// * `websocket_port` - The WebSocket port to expose (default: 3001)
/// * `env_vars` - Additional environment variables to include
///
/// # Returns
///
/// The path to the created Docker Compose file
pub fn write_docker_compose_file(
    agent_dir: &Path,
    agent_id: &str,
    http_port: Option<u16>,
    websocket_port: Option<u16>,
    env_vars: HashMap<String, String>,
) -> Result<PathBuf, String> {
    // Create the Docker Compose config
    let (_, yaml) = create_agent_compose_config(agent_id, http_port, websocket_port, env_vars);

    // Write the Docker Compose file
    let compose_path = agent_dir.join("docker-compose.yml");
    fs::write(&compose_path, yaml)
        .map_err(|e| format!("Failed to write docker-compose.yml: {}", e))?;

    Ok(compose_path)
}

/// Initializes a TeeDeployer with the provided API credentials
///
/// # Arguments
///
/// * `api_key` - The Phala TEE API key
/// * `api_endpoint` - The Phala TEE API endpoint
///
/// # Returns
///
/// A Result containing the initialized TeeDeployer or an error string
pub fn init_tee_deployer(api_key: &str, api_endpoint: &str) -> Result<TeeDeployer, String> {
    TeeDeployerBuilder::new()
        .with_api_key(api_key.to_string())
        .with_api_endpoint(api_endpoint.to_string())
        .build()
        .map_err(|e| format!("Failed to initialize TeeDeployer: {}", e))
}

/// Clean up Docker containers by name pattern
///
/// # Arguments
///
/// * `name_pattern` - Pattern to match container names (e.g., "coinbase-agent-")
///
/// # Returns
///
/// The number of containers removed
pub fn cleanup_containers(name_pattern: &str) -> u32 {
    let output = std::process::Command::new("docker")
        .args([
            "ps",
            "-aq",
            "--filter",
            &format!("name={}", name_pattern),
            "--format",
            "{{.ID}}",
        ])
        .output();

    match output {
        Ok(output) => {
            if !output.stdout.is_empty() {
                let container_ids = String::from_utf8_lossy(&output.stdout);
                let mut count = 0;

                for id in container_ids.trim().split('\n') {
                    if !id.is_empty() {
                        if let Ok(rm_output) = std::process::Command::new("docker")
                            .args(["rm", "-f", id])
                            .output()
                        {
                            if rm_output.status.success() {
                                count += 1;
                            }
                        }
                    }
                }

                count
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}
