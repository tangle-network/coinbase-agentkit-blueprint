use crate::docker;
use crate::types::{AgentDeploymentResult, DeployAgentParams};
use crate::ServiceContext;
use blueprint_sdk::logging;
use dotenv::dotenv;
use phala_tee_deploy_rs::Encryptor;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;

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

    // Get the encrypted environment variables
    let encrypted_env = params
        .encrypted_env_vars
        .as_ref()
        .ok_or_else(|| "No encrypted environment variables provided".to_string())?;

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
    logging::info!("Deploying agent to TEE with encrypted environment variables");
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

    // Get the PORT from env var or fallback to 3000
    let port = read_port_from_env(agent_dir).unwrap_or(3000);
    let websocket_port = port + 1; // Websocket port is typically HTTP port + 1

    // Create a unique container name using agent ID
    let container_name = format!("coinbase-agent-{}", params.agent_id);

    // Clean up any existing containers using docker-compose down
    let _ = tokio::process::Command::new("docker-compose")
        .args(&["down", "--remove-orphans"])
        .current_dir(agent_dir)
        .output()
        .await;

    // Create a .env file in the agent directory with all required variables
    let env_file_path = agent_dir.join(".env");
    let mut env_content = String::new();

    // Add basic configuration
    env_content.push_str(&format!("PORT={}\n", port));
    env_content.push_str(&format!("WEBSOCKET_PORT={}\n", websocket_port));
    env_content.push_str(&format!("CONTAINER_NAME={}\n", container_name));
    env_content.push_str("NODE_ENV=development\n");
    env_content.push_str("AGENT_MODE=http\n"); // Support both http and websocket
    env_content.push_str("MODEL=gpt-4o-mini\n");
    env_content.push_str("LOG_LEVEL=debug\n");
    env_content.push_str(&format!(
        "WEBSOCKET_URL=ws://localhost:{}\n",
        websocket_port
    ));

    // Add OpenAI API key (try from params first, then environment)
    let openai_api_key = match &params.api_key_config {
        Some(config) if config.openai_api_key.is_some() => config.openai_api_key.clone().unwrap(),
        _ => {
            std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "dummy-key-for-testing".to_string())
        }
    };

    env_content.push_str(&format!("OPENAI_API_KEY={}\n", openai_api_key));

    // Add wallet variables with test values (for testing purposes)
    env_content.push_str("CDP_API_KEY_NAME=test-key\n");
    env_content.push_str("CDP_API_KEY_PRIVATE_KEY=test-private-key\n");

    // Include RUN_TESTS flag to disable tests during build
    env_content.push_str("RUN_TESTS=false\n");

    // Write the .env file
    fs::write(&env_file_path, env_content)
        .map_err(|e| format!("Failed to write .env file: {}", e))?;

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

    // Get the container IP address
    let container_ip = get_container_ip(&container_name).await?;
    let endpoint = format!("http://{}:{}", container_ip, port);

    // Use the AgentEndpoint helper to check if the agent is healthy
    logging::info!(
        "Waiting for agent to become healthy at endpoint: {}",
        endpoint
    );
    let agent = docker::AgentEndpoint::new(&endpoint);

    // Try to wait for the agent to become healthy with reasonable timeouts
    match agent
        .wait_for_health(
            10,
            std::time::Duration::from_millis(500),
            std::time::Duration::from_secs(2),
        )
        .await
    {
        Ok(_) => logging::info!("Agent is healthy and ready for use"),
        Err(e) => logging::warn!(
            "Agent health check timed out, but continuing deployment: {}",
            e
        ),
    }

    logging::info!("Agent deployed locally. Endpoint: {}", endpoint);

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

/// Read the PORT from the .env file
fn read_port_from_env(agent_dir: &Path) -> Option<u16> {
    let env_file_path = agent_dir.join(".env");
    if let Ok(content) = fs::read_to_string(&env_file_path) {
        for line in content.lines() {
            if line.starts_with("AGENT_PORT=") || line.starts_with("PORT=") {
                if let Some(port_str) = line.split('=').nth(1) {
                    if let Ok(port) = port_str.trim().parse::<u16>() {
                        return Some(port);
                    }
                }
            }
        }
    }
    None
}

/// Get the IP address of a Docker container
async fn get_container_ip(container_name: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("docker")
        .args(&[
            "inspect",
            "-f",
            "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
            container_name,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to get container IP: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to get container IP: {}", stderr));
    }

    let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if ip.is_empty() {
        return Err("Container IP not found".to_string());
    }

    Ok(ip)
}
