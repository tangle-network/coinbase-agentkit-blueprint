use crate::docker;
use crate::helpers::{check_agent_health, get_container_logs};
use crate::types::{AgentDeploymentResult, DeployAgentParams};
use crate::ServiceContext;
use blueprint_sdk::logging;
use dotenv::dotenv;
use serde_json;
use std::fs;
use std::path::Path;
use tokio::process::Command as TokioCommand;

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

    // Check if this is a TEE deployment - use context directly
    let tee_enabled = context.tee_enabled.unwrap_or(false);

    if tee_enabled {
        // Deploy to TEE
        deploy_to_tee(&agent_dir, &params, context).await
    } else {
        // Deploy locally with Docker
        deploy_locally(&agent_dir, &params, context).await
    }
}

/// Deploy the agent to Phala TEE using TeeDeployer
async fn deploy_to_tee(
    agent_dir: &Path,
    params: &DeployAgentParams,
    context: &ServiceContext,
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

    logging::info!(
        "Deploying agent to TEE with Docker compose: {:#?}",
        docker_compose
    );

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
    let app_name = format!("coinbase-agent-{}", params.agent_id);
    let vm_config = deployer
        .create_vm_config_from_string(
            &docker_compose,
            &app_name,
            Some(2),    // vcpu
            Some(2048), // memory in MB
            Some(10),   // disk size in GB
        )
        .map_err(|e| format!("Failed to create VM configuration: {}", e))?;

    logging::info!(
        "Deploying agent to TEE with VM configuration: {:#?}",
        vm_config
    );

    // Get the public key for this VM configuration
    logging::info!("Requesting encryption public key...");
    let vm_config_json = serde_json::to_value(&vm_config).unwrap();
    let pubkey_response = deployer
        .get_pubkey_for_config(&vm_config_json)
        .await
        .map_err(|e| format!("Failed to get TEE public key: {}", e))?;

    logging::info!(
        "Deploying agent to TEE with pubkey response: {:#?}",
        pubkey_response
    );

    let pubkey = pubkey_response.app_env_encrypt_pubkey;
    let salt = pubkey_response.app_id_salt;

    // Deploy with the VM configuration and encrypted environment variables
    logging::info!("Deploying agent to TEE with encrypted environment variables");
    let vm_config_json = serde_json::to_value(&vm_config).unwrap();
    let deployment = deployer
        .deploy_with_encrypted_env(vm_config_json, encrypted_env.clone(), &pubkey, &salt)
        .await
        .map_err(|e| format!("Failed to deploy to TEE: {}", e))?;

    logging::info!("TEE deployment completed. Deployment: {:#?}", deployment);

    // Prepare the deployment result
    let result = AgentDeploymentResult {
        agent_id: params.agent_id.clone(),
        endpoint: None,
        tee_pubkey: Some(pubkey),
        tee_salt: Some(salt),
    };

    // Serialize the result
    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}

/// Deploy the agent locally using Docker Compose
async fn deploy_locally(
    agent_dir: &Path,
    params: &DeployAgentParams,
    context: &ServiceContext,
) -> Result<Vec<u8>, String> {
    // Load .env file if it exists
    dotenv().ok();

    // Create a unique container name using agent ID
    let container_name = format!("coinbase-agent-{}", params.agent_id);
    logging::info!("Using container name: {}", container_name);

    // Get port configuration - strict checking from context
    let (http_port, websocket_port) = get_required_ports(&params.agent_id, context)?;
    logging::info!(
        "Using ports - HTTP: {}, WebSocket: {}",
        http_port,
        websocket_port
    );

    // Note: Container cleanup is now expected to be handled by the tests

    // Create a .env file with required configurations
    let env_file_path = agent_dir.join(".env");
    logging::info!("Creating .env file at: {}", env_file_path.display());
    let env_content = create_env_content(http_port, websocket_port, &container_name, params)?;

    // Write the .env file
    fs::write(&env_file_path, env_content)
        .map_err(|e| format!("Failed to write .env file: {}", e))?;
    logging::info!(".env file written successfully");

    // Verify docker-compose.yml exists
    let compose_path = agent_dir.join("docker-compose.yml");
    if !compose_path.exists() {
        return Err(format!(
            "docker-compose.yml not found at {}",
            compose_path.display()
        ));
    }

    // Start the Docker container
    logging::info!("Starting Docker container");
    let output = TokioCommand::new("docker-compose")
        .args(&["up", "-d"])
        .current_dir(agent_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to start Docker container: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to start Docker container: {}", stderr));
    }
    logging::info!("Container started successfully");

    // For local deployments, use localhost
    let endpoint = format!("http://localhost:{}", http_port);

    // Check if the agent is healthy - this function now includes initial delay and retry logic
    if let Err(health_error) = check_agent_health(&endpoint).await {
        logging::error!("Agent health check failed: {}", health_error);

        // Get container logs for diagnosis - note: this is a synchronous function
        match get_container_logs(&container_name) {
            Ok(logs) => {
                logging::error!("Container logs:");
                // Split and log each line individually for better readability in logs
                for line in logs.lines().take(20) {
                    logging::error!("  | {}", line);
                }
            }
            Err(e) => logging::error!("Failed to get logs: {}", e),
        }

        return Err(format!("Deployment failed: {}", health_error));
    }

    logging::info!("Agent is healthy and ready for use at {}", endpoint);

    // Prepare the deployment result
    let result = AgentDeploymentResult {
        agent_id: params.agent_id.clone(),
        endpoint: Some(endpoint),
        tee_pubkey: None,
        tee_salt: None,
    };

    // Serialize the result
    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}

/// Get required ports from context
fn get_required_ports(agent_id: &str, context: &ServiceContext) -> Result<(u16, u16), String> {
    // Only get ports from the agent_ports map in context
    if let Some(agent_ports) = &context.agent_ports {
        if let Ok(ports_map) = agent_ports.lock() {
            if let Some(port_config) = ports_map.get(agent_id) {
                return Ok((port_config.http_port, port_config.websocket_port));
            }
        }
    }

    // If we get here, no ports were found
    Err(format!(
        "No port configuration found for agent {}",
        agent_id
    ))
}

/// Helper function to create the environment content for the agent
fn create_env_content(
    port: u16,
    websocket_port: u16,
    container_name: &str,
    params: &DeployAgentParams,
) -> Result<String, String> {
    // Get API config or fail early
    let api_config = params
        .api_key_config
        .as_ref()
        .ok_or_else(|| "API key configuration is required".to_string())?;

    // Get required API keys or fail
    let openai_api_key = api_config
        .openai_api_key
        .as_ref()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .ok_or_else(|| "OPENAI_API_KEY not found in config or environment".to_string())?;

    let cdp_api_key_name = api_config
        .cdp_api_key_name
        .as_ref()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CDP_API_KEY_NAME").ok())
        .ok_or_else(|| "CDP_API_KEY_NAME not found in config or environment".to_string())?;

    let cdp_api_key_private_key = api_config
        .cdp_api_key_private_key
        .as_ref()
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

    // Build environment content with all required variables
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

    Ok(env_content)
}
