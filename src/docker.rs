use dockworker::config::compose::{BuildConfig, ComposeConfig, Service};
use dockworker::config::EnvironmentVars;
use phala_tee_deploy_rs::{TeeDeployer, TeeDeployerBuilder};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
      - LOG_LEVEL=info
    restart: unless-stopped
"#,
        http_port, http_port, websocket_port, websocket_port, http_port, websocket_port, agent_id
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

/// A struct representing a deployed agent endpoint
#[derive(Debug, Clone)]
pub struct AgentEndpoint {
    /// Base URL of the agent (e.g., http://localhost:3000)
    pub base_url: String,
    /// HTTP client for making requests
    http_client: reqwest::Client,
}

impl AgentEndpoint {
    /// Creates a new AgentEndpoint
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the agent (e.g., "http://localhost:3000")
    ///
    /// # Returns
    ///
    /// A new AgentEndpoint instance
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Creates an AgentEndpoint from a port number (localhost)
    ///
    /// # Arguments
    ///
    /// * `port` - The HTTP port the agent is listening on
    ///
    /// # Returns
    ///
    /// A new AgentEndpoint instance
    pub fn from_port(port: u16) -> Self {
        Self::new(format!("http://localhost:{}", port))
    }

    /// Checks if the agent's health endpoint is responding
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for a response
    ///
    /// # Returns
    ///
    /// A Result containing the health status or an error
    pub async fn check_health(&self, timeout: Duration) -> Result<Value, String> {
        let health_url = format!("{}/health", self.base_url);
        self.http_client
            .get(&health_url)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| format!("Health check request failed: {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse health response: {}", e))
    }

    /// Sends a message to the agent and gets a response
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send to the agent
    /// * `timeout` - Maximum time to wait for a response
    ///
    /// # Returns
    ///
    /// A Result containing the agent's response or an error
    pub async fn interact(&self, message: &str, timeout: Duration) -> Result<Value, String> {
        let interact_url = format!("{}/interact", self.base_url);
        self.http_client
            .post(&interact_url)
            .json(&json!({ "message": message }))
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| format!("Interaction request failed: {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse interaction response: {}", e))
    }

    /// Waits for the agent to become healthy with retries
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - Maximum number of health check attempts
    /// * `initial_delay` - Initial delay between attempts
    /// * `timeout` - Timeout for each health check request
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error after max attempts
    pub async fn wait_for_health(
        &self,
        max_attempts: u32,
        initial_delay: Duration,
        timeout: Duration,
    ) -> Result<(), String> {
        let mut delay = initial_delay;
        let max_delay = Duration::from_secs(2); // Cap maximum delay at 2 seconds

        for attempt in 1..=max_attempts {
            match self.check_health(timeout).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if attempt == max_attempts {
                        return Err(format!(
                            "Agent failed to become healthy after {} attempts: {}",
                            max_attempts, e
                        ));
                    }

                    // Log the error and retry after delay
                    eprintln!("Health check attempt {} failed: {}", attempt, e);
                    tokio::time::sleep(delay).await;

                    // Exponential backoff with a cap
                    delay = std::cmp::min(delay * 2, max_delay);
                }
            }
        }

        // Shouldn't reach here due to early return, but just in case
        Err("Failed to verify agent health".to_string())
    }
}

/// Enum representing the type of deployment (local Docker or TEE)
#[derive(Debug, Clone, PartialEq)]
pub enum DeploymentType {
    /// Local Docker deployment
    Local,
    /// TEE deployment
    Tee,
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

/// Create a helper function for testing with a mock TEE deployer
#[cfg(test)]
pub fn create_mock_tee_deployer(mock_teepod_id: u64) -> Result<TeeDeployer, String> {
    let mock_deployer = TeeDeployerBuilder::new()
        .with_api_key("mock_api_key".to_string())
        .with_api_endpoint("https://cloud-api.phala.network/api/v1".to_string())
        .build()
        .map_err(|e| format!("Failed to create mock TeeDeployer: {}", e))?;

    // For testing only, directly set the TEEPod ID using the TeeDeployer's internal API
    #[allow(deprecated)]
    mock_deployer
        .select_teepod(mock_teepod_id)
        .map_err(|e| format!("Failed to set mock TEEPod ID: {}", e))?;

    Ok(mock_deployer)
}
