use crate::docker;
use crate::helpers::{check_agent_health, collect_container_diagnostics, inspect_container_env};
use crate::types::{AgentDeploymentResult, DeployAgentParams};
use crate::ServiceContext;
use blueprint_sdk::logging;
use dockworker::ComposeConfig;
use dotenv::dotenv;
use serde_json;
use serde_yaml;
use std::fs;
use std::path::Path;
use uuid::Uuid;
use url;
use reqwest;

/// Handles the deploy_agent job
pub async fn handle_deploy_agent(
    params_bytes: Vec<u8>,
    context: &ServiceContext,
) -> Result<Vec<u8>, String> {
    // Deserialize the parameters from bytes
    let params: DeployAgentParams = match serde_json::from_slice(&params_bytes) {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to deserialize parameters: {}", e)),
    };

    // Define base directory from context or environment
    let base_dir = match &context.agents_base_dir {
        Some(dir) => dir.clone(),
        None => "./agents".to_string(),
    };

    // Check if agent directory exists
    let agent_dir = Path::new(&base_dir).join(&params.agent_id);
    if !agent_dir.exists() {
        return Err(format!(
            "Agent directory does not exist: {}",
            agent_dir.display()
        ));
    }

    // Generate a unique deployment ID
    let deployment_id = Uuid::new_v4().to_string();

    // Check if this is a TEE deployment - use context directly
    let tee_enabled = context.tee_enabled.unwrap_or(false);

    if tee_enabled {
        // Deploy to TEE
        deploy_to_tee(&agent_dir, &params, context, &deployment_id).await
    } else {
        // Deploy locally with Docker
        deploy_locally(&agent_dir, &params, &deployment_id, context).await
    }
}

/// Deploy the agent to Phala TEE using TeeDeployer
async fn deploy_to_tee(
    agent_dir: &Path,
    params: &DeployAgentParams,
    context: &ServiceContext,
    deployment_id: &str,
) -> Result<Vec<u8>, String> {
    // Get API key directly from context
    let tee_api_key = context
        .phala_tee_api_key
        .as_ref()
        .ok_or("PHALA_CLOUD_API_KEY not set")?;

    // Get API endpoint from environment
    let tee_api_endpoint = context
        .phala_tee_api_endpoint
        .as_ref()
        .ok_or("PHALA_CLOUD_API_ENDPOINT not set")?;

    // Read docker-compose.yml from the agent directory
    let docker_compose_path = agent_dir.join("docker-compose.yml");
    let docker_compose = fs::read_to_string(&docker_compose_path)
        .map_err(|e| format!("Failed to read docker-compose.yml: {}", e))?;

    // Initialize the TeeDeployer
    logging::info!("Initializing TeeDeployer for deployment");
    let mut deployer = docker::init_tee_deployer(tee_api_key, tee_api_endpoint)?;

    // Discover an available TEEPod
    logging::info!("Discovering available TEEPods...");
    deployer
        .discover_teepod()
        .await
        .map_err(|e| format!("Failed to discover TEEPods: {}", e))?;

    // Get the encrypted environment variables - they are already encrypted properly
    let encrypted_env = params.encrypted_env.as_ref().ok_or_else(|| {
        "No encrypted environment variables provided for TEE deployment".to_string()
    })?;

    // Create VM configuration using TeeDeployer's native method
    logging::info!("Creating VM configuration from Docker Compose");
    let app_name = format!("coinbase-agent-{}", deployment_id);
    let vm_config = deployer
        .create_vm_config_from_string(
            &docker_compose,
            &app_name,
            Some(2),    // vcpu
            Some(2048), // memory in MB
            Some(10),   // disk size in GB
        )
        .map_err(|e| format!("Failed to create VM configuration: {}", e))?;

    // Get the public key for this VM configuration
    logging::info!("Requesting encryption public key...");
    let pubkey_response = deployer
        .get_pubkey_for_config(&vm_config)
        .await
        .map_err(|e| format!("Failed to get TEE public key: {}", e))?;

    let pubkey = pubkey_response["app_env_encrypt_pubkey"]
        .as_str()
        .ok_or_else(|| "Missing public key in response".to_string())?;

    let salt = pubkey_response["app_id_salt"]
        .as_str()
        .ok_or_else(|| "Missing salt in response".to_string())?;

    // Deploy with the VM configuration and encrypted environment variables
    logging::info!("Deploying agent to TEE with pre-encrypted environment variables");
    let deployment = deployer
        .deploy_with_encrypted_env(vm_config, encrypted_env.clone(), pubkey, salt)
        .await
        .map_err(|e| format!("Failed to deploy to TEE: {}", e))?;

    // Extract endpoint and app_id from deployment
    let endpoint = deployment["endpoint"].as_str().map(|s| s.to_string());
    let app_id = deployment["id"].as_str().map(|s| s.to_string());

    logging::info!(
        "TEE deployment completed. Endpoint: {:?}, App ID: {:?}",
        endpoint,
        app_id
    );

    // Prepare the deployment result
    let result = AgentDeploymentResult {
        agent_id: params.agent_id.clone(),
        deployment_id: deployment_id.to_string(),
        endpoint,
        tee_pubkey: None, // Already provided during creation
        tee_app_id: app_id,
    };

    // Serialize the result
    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}

/// Deploy the agent locally using Docker Compose
async fn deploy_locally(
    agent_dir: &Path,
    params: &DeployAgentParams,
    deployment_id: &str,
    context: &ServiceContext,
) -> Result<Vec<u8>, String> {
    // Load .env file from the current directory if it exists
    dotenv().ok();

    // Create a unique container name using agent ID
    let container_name = format!("coinbase-agent-{}", params.agent_id);
    logging::info!("Using container name: {}", container_name);

    // Clean up any existing containers using docker-compose down
    logging::info!("Cleaning up any existing containers...");
    let cleanup_output = tokio::process::Command::new("docker-compose")
        .args(&["down", "--remove-orphans"])
        .current_dir(agent_dir)
        .output()
        .await;

    if let Ok(output) = &cleanup_output {
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            logging::warn!("Cleanup warning (non-critical): {}", stderr);
        }
    }

    // Get port configuration - first try the agent_ports map, then fall back to docker-compose
    let (port, websocket_port) = get_agent_ports(agent_dir, &params.agent_id, context)?;
    logging::info!(
        "Using ports - HTTP: {}, WebSocket: {}",
        port,
        websocket_port
    );

    // Create a .env file in the agent directory with all required variables
    let env_file_path = agent_dir.join(".env");
    logging::info!("Creating .env file at: {}", env_file_path.display());
    let env_content = create_env_content(port, websocket_port, &container_name, params)?;

    // Log the environment variables (excluding sensitive values)
    logging::info!("Environment variables prepared (sensitive values redacted):");
    for line in env_content.lines() {
        if line.contains("API_KEY") || line.contains("PRIVATE_KEY") {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                logging::info!("  {}=***REDACTED***", parts[0]);
            }
        } else {
            logging::info!("  {}", line);
        }
    }

    // Write the .env file
    fs::write(&env_file_path, env_content)
        .map_err(|e| format!("Failed to write .env file: {}", e))?;
    logging::info!(".env file written successfully");

    // Check docker-compose.yml exists
    let compose_path = agent_dir.join("docker-compose.yml");
    if !compose_path.exists() {
        return Err(format!(
            "docker-compose.yml not found at {}",
            compose_path.display()
        ));
    }
    logging::info!("docker-compose.yml found at: {}", compose_path.display());

    // Start the Docker container using docker-compose
    logging::info!("Starting Docker container for agent: {}", params.agent_id);
    let output = tokio::process::Command::new("docker-compose")
        .args(&["up", "-d"])
        .current_dir(agent_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to start Docker container: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        logging::error!("Docker compose error: {}", stderr);
        return Err(format!("Failed to start Docker container: {}", stderr));
    }

    logging::info!("Docker compose up command executed successfully");

    // Verify the container is running
    logging::info!("Verifying container status...");
    let ps_output = tokio::process::Command::new("docker-compose")
        .args(&["ps"])
        .current_dir(agent_dir)
        .output()
        .await;

    if let Ok(output) = ps_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        logging::info!("Docker compose ps output:\n{}", stdout);
    }

    // Check Docker logs to see what's happening inside the container
    logging::info!("Checking container logs...");
    let _ = tokio::process::Command::new("docker-compose")
        .args(&["logs"])
        .current_dir(agent_dir)
        .output()
        .await
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            logging::info!("Docker container logs:\n{}", stdout);
        });

    // For local deployments, we always use localhost
    let endpoint = format!("http://localhost:{}", port);
    logging::info!("Agent deployed with endpoint: {}", endpoint);

    // Wait for the container to start and collect diagnostics
    let diagnostics_result = collect_container_diagnostics(&container_name, port).await;
    match &diagnostics_result {
        Ok(details) => logging::info!("Container diagnostics: {}", details),
        Err(e) => logging::warn!("Failed to collect container diagnostics: {}", e),
    }

    // Check if the agent is healthy
    match check_agent_health(&endpoint).await {
        Ok(_) => logging::info!("Agent is healthy and ready for use"),
        Err(e) => {
            // Log error but continue - we'll return the deployment result anyway
            logging::error!("Agent health check failed: {}", e);

            // Get detailed diagnostics if the first attempt failed
            if diagnostics_result.is_err() {
                let _ = collect_container_diagnostics(&container_name, port).await
                    .map(|details| logging::info!("Additional container diagnostics after health check failure: {}", details));
            }
            
            // Check logs again after health check failure
            let _ = tokio::process::Command::new("docker")
                .args(&["logs", &container_name])
                .output()
                .await
                .map(|output| {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    logging::info!("Latest container logs after health check failure:\n{}", stdout);
                    
                    // Check for specific error patterns in logs
                    if stdout.contains("Failed to initialize wallet") {
                        logging::error!("DETECTED ERROR: Wallet initialization failed - CDP credentials may be invalid");
                        
                        // Inspect container environment to verify CDP credentials were passed correctly
                        tokio::spawn(async move {
                            match inspect_container_env(&container_name).await {
                                Ok(env_info) => logging::info!("Container environment inspection:\n{}", env_info),
                                Err(e) => logging::error!("Failed to inspect container environment: {}", e),
                            }
                        });
                    } else if stdout.contains("EADDRINUSE") {
                        logging::error!("DETECTED ERROR: Port already in use - container cannot bind to the required port");
                    } else if stdout.contains("Out of memory") || stdout.contains("Killed") {
                        logging::error!("DETECTED ERROR: Container terminated due to memory constraints");
                    }
                });
        }
    };

    // Prepare the deployment result
    let result = AgentDeploymentResult {
        agent_id: params.agent_id.clone(),
        deployment_id: deployment_id.to_string(),
        endpoint: Some(endpoint),
        tee_pubkey: None,
        tee_app_id: None,
    };

    // Serialize the result
    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}


/// Helper function to get agent ports - either from context or extracted from docker-compose
fn get_agent_ports(
    agent_dir: &Path,
    agent_id: &str,
    context: &ServiceContext,
) -> Result<(u16, u16), String> {
    // Try to get ports from the agent_ports map
    if let Some(agent_ports) = &context.agent_ports {
        if let Ok(ports_map) = agent_ports.lock() {
            if let Some(port_config) = ports_map.get(agent_id) {
                logging::info!(
                    "Using stored port configuration for agent {}: HTTP:{}, WS:{}",
                    agent_id,
                    port_config.http_port,
                    port_config.websocket_port
                );
                return Ok((port_config.http_port, port_config.websocket_port));
            }
        }
    }

    // Fall back to extracting from docker-compose
    logging::info!(
        "Extracting ports from docker-compose for agent {}",
        agent_id
    );
    extract_ports_from_compose(agent_dir)
}

/// Helper function to create the environment content for the agent
fn create_env_content(
    port: u16,
    websocket_port: u16,
    container_name: &str,
    params: &DeployAgentParams,
) -> Result<String, String> {
    // Get API config or fail early
    let api_config = params.api_key_config.as_ref()
        .ok_or_else(|| "API key configuration is required".to_string())?;

    // Get required API keys or fail
    let openai_api_key = api_config.openai_api_key.as_ref()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .ok_or_else(|| "OPENAI_API_KEY not found in config or environment".to_string())?;

    let cdp_api_key_name = api_config.cdp_api_key_name.as_ref()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CDP_API_KEY_NAME").ok())
        .ok_or_else(|| "CDP_API_KEY_NAME not found in config or environment".to_string())?;

    let cdp_api_key_private_key = api_config.cdp_api_key_private_key.as_ref()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CDP_API_KEY_PRIVATE_KEY").ok())
        .ok_or_else(|| "CDP_API_KEY_PRIVATE_KEY not found in config or environment".to_string())?;

    // Validate keys are not empty
    if cdp_api_key_name.trim().is_empty() {
        return Err("CDP_API_KEY_NAME is empty".to_string());
    }
    if cdp_api_key_private_key.trim().is_empty() {
        return Err("CDP_API_KEY_PRIVATE_KEY is empty".to_string());
    }

    // Build environment content
    let env_content = format!(
        "PORT={port}\n\
         WEBSOCKET_PORT={websocket_port}\n\
         CONTAINER_NAME={container_name}\n\
         NODE_ENV=development\n\
         AGENT_MODE=http\n\
         MODEL=gpt-4o-mini\n\
         LOG_LEVEL=debug\n\
         WEBSOCKET_URL=ws://localhost:{websocket_port}\n\
         OPENAI_API_KEY={openai_api_key}\n\
         CDP_API_KEY_NAME={cdp_api_key_name}\n\
         CDP_API_KEY_PRIVATE_KEY={cdp_api_key_private_key}\n\
         RUN_TESTS=false\n"
    );

    logging::info!("Environment content created with {} variables", env_content.lines().count());
    Ok(env_content)
}

/// Helper function to extract port configuration from docker-compose.yml
fn extract_ports_from_compose(agent_dir: &Path) -> Result<(u16, u16), String> {
    extract_port_config(agent_dir.join("docker-compose.yml"))
}

/// Extract HTTP and WebSocket port configuration from a Docker Compose file
///
/// This function parses a Docker Compose file and extracts the HTTP port from the first
/// service's port mapping. The WebSocket port is assumed to be the HTTP port + 1.
///
/// # Arguments
///
/// * `compose_path` - Path to the docker-compose.yml file
///
/// # Returns
///
/// * `Result<(u16, u16), String>` - A tuple of (http_port, websocket_port) or an error
pub fn extract_port_config(compose_path: impl AsRef<Path>) -> Result<(u16, u16), String> {
    // Read the docker-compose.yml
    let docker_compose_content = fs::read_to_string(&compose_path)
        .map_err(|e| format!("Failed to read docker-compose.yml: {}", e))?;

    // Parse the docker-compose.yml to extract port mapping
    let compose_config: ComposeConfig = serde_yaml::from_str(&docker_compose_content)
        .map_err(|e| format!("Failed to parse docker-compose.yml: {}", e))?;

    // Extract the port mapping from the first service in the compose file
    let (service_name, service) = compose_config
        .services
        .iter()
        .next()
        .ok_or_else(|| "No services found in docker-compose.yml".to_string())?;

    logging::info!("Extracting ports from service: {}", service_name);

    // Look for port mapping in the service's ports section
    let http_port = match &service.ports {
        Some(ports) if !ports.is_empty() => {
            // Extract the first port mapping (format usually "HOST:CONTAINER")
            let port_mapping = &ports[0];
            if let Some(colon_pos) = port_mapping.find(':') {
                // Parse the host port
                let host_port = &port_mapping[0..colon_pos];
                host_port.parse::<u16>().map_err(|e| {
                    format!("Failed to parse host port from '{}': {}", port_mapping, e)
                })?
            } else {
                // If no colon, assume it's just the container port mapped to same host port
                port_mapping
                    .parse::<u16>()
                    .map_err(|e| format!("Failed to parse port from '{}': {}", port_mapping, e))?
            }
        }
        _ => {
            logging::warn!("No ports specified in docker-compose.yml, using default port 3000");
            3000 // Default if no ports specified
        }
    };

    logging::info!("Extracted HTTP port {} from docker-compose.yml", http_port);
    let websocket_port = http_port + 1; // Websocket port is typically HTTP port + 1
    logging::info!("Using WebSocket port {}", websocket_port);

    Ok((http_port, websocket_port))
}
