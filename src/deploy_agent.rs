use crate::types::{AgentDeploymentResult, DeployAgentParams};
use crate::ServiceContext;
use phala_tee_deploy_rs::{DeploymentConfig as TeeDeployConfig, TeeClient};
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
    let base_dir = context
        .get_env_var("AGENT_BASE_DIR")
        .unwrap_or_else(|| "./agents".to_string());

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

    // Check if this is a TEE deployment
    let tee_enabled = context
        .get_env_var("TEE_ENABLED")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    if tee_enabled {
        // Deploy to TEE
        deploy_to_tee(&agent_dir, &params, context, &deployment_id).await
    } else {
        // Deploy locally with Docker
        deploy_locally(&agent_dir, &params, &deployment_id, context).await
    }
}

/// Deploy the agent to Phala TEE
async fn deploy_to_tee(
    agent_dir: &Path,
    params: &DeployAgentParams,
    context: &ServiceContext,
    deployment_id: &str,
) -> Result<Vec<u8>, String> {
    // Get TEE configuration
    let tee_api_key = context
        .get_env_var("PHALA_CLOUD_API_KEY")
        .ok_or_else(|| "PHALA_CLOUD_API_KEY not set".to_string())?;

    let teepod_id = context
        .get_env_var("PHALA_TEEPOD_ID")
        .ok_or_else(|| "PHALA_TEEPOD_ID not set".to_string())?
        .parse::<u64>()
        .map_err(|e| format!("Invalid PHALA_TEEPOD_ID: {}", e))?;

    // Read docker-compose.yml from the template
    let docker_compose_path = agent_dir.join("docker-compose.yml");
    let docker_compose = fs::read_to_string(&docker_compose_path)
        .map_err(|e| format!("Failed to read docker-compose.yml: {}", e))?;

    // Create TEE deployment config
    let tee_config = TeeDeployConfig::new(
        tee_api_key.clone(),
        docker_compose,
        HashMap::new(),
        teepod_id,
        "phala-worker:latest".to_string(),
    );

    // Initialize TEE client
    let client = TeeClient::new(tee_config)
        .map_err(|e| format!("Failed to initialize TEE client: {}", e))?;

    // Get the encrypted environment variables
    let encrypted_env = params
        .encrypted_env_vars
        .as_ref()
        .ok_or_else(|| "No encrypted environment variables provided".to_string())?;

    // Get the public key for encryption
    let vm_config = serde_json::json!({
        "teepod_id": teepod_id,
        "image": "phala-worker:latest"
    });

    let pubkey_response = client
        .get_pubkey_for_config(&vm_config)
        .await
        .map_err(|e| format!("Failed to get TEE public key: {}", e))?;

    let pubkey = pubkey_response["app_env_encrypt_pubkey"]
        .as_str()
        .ok_or_else(|| "Invalid public key response".to_string())?;

    // Deploy to TEE with encrypted environment variables
    let deployment = client
        .deploy_with_config_encrypted_env(vm_config, encrypted_env.clone(), pubkey, &deployment_id)
        .await
        .map_err(|e| format!("Failed to deploy to TEE: {}", e))?;

    // Extract endpoint from deployment details
    let endpoint = deployment
        .details
        .as_ref()
        .map(|d| d.get("endpoint").map(|v| v.to_string()));
    let app_id = deployment
        .details
        .as_ref()
        .map(|d| d.get("app_id").map(|v| v.to_string()));

    // Prepare the deployment result
    let result = AgentDeploymentResult {
        agent_id: params.agent_id.clone(),
        deployment_id: deployment_id.to_string(),
        endpoint: endpoint.flatten(),
        tee_pubkey: None, // Already provided during creation
        tee_app_id: app_id.flatten(),
    };

    // Serialize the result
    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}

/// Deploy the agent locally using Docker
async fn deploy_locally(
    agent_dir: &Path,
    params: &DeployAgentParams,
    deployment_id: &str,
    context: &ServiceContext,
) -> Result<Vec<u8>, String> {
    // Update environment variables if provided
    if let Some(api_key_config) = &params.api_key_config {
        update_env_file(agent_dir, api_key_config)?;
    }

    // Deploy using docker-compose from the template directory
    let status = tokio::process::Command::new("docker-compose")
        .current_dir(agent_dir)
        .arg("up")
        .arg("-d")
        .status()
        .await
        .map_err(|e| format!("Failed to execute docker-compose: {}", e))?;

    if !status.success() {
        return Err("Failed to deploy agent with docker-compose".to_string());
    }

    // Get the host and port for the endpoint
    let host = context
        .get_env_var("SERVER_HOST")
        .unwrap_or_else(|| "localhost".to_string());
    let port = context
        .get_env_var("HTTP_PORT")
        .map(|p| p.parse::<u16>().unwrap_or(3000))
        .unwrap_or(3000);

    // Prepare the deployment result
    let result = AgentDeploymentResult {
        agent_id: params.agent_id.clone(),
        deployment_id: deployment_id.to_string(),
        endpoint: Some(format!("http://{}:{}", host, port)),
        tee_pubkey: None,
        tee_app_id: None,
    };

    // Serialize the result
    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}

/// Updates the .env file with new API keys
fn update_env_file(
    agent_dir: &Path,
    api_key_config: &crate::types::ApiKeyConfig,
) -> Result<(), String> {
    let env_path = agent_dir.join(".env");

    // Read existing .env file
    let mut env_content = match fs::read_to_string(&env_path) {
        Ok(content) => content,
        Err(e) => return Err(format!("Failed to read .env file: {}", e)),
    };

    // Update OpenAI API key if provided
    if let Some(openai_key) = &api_key_config.openai_api_key {
        update_env_var(&mut env_content, "OPENAI_API_KEY", openai_key);
    }

    // Write updated .env file
    fs::write(&env_path, env_content)
        .map_err(|e| format!("Failed to write updated .env file: {}", e))
}

/// Updates a specific environment variable in the .env content
fn update_env_var(env_content: &mut String, key: &str, value: &str) {
    // Check if the key exists in the file
    if env_content.contains(&format!("{}=", key)) {
        // Replace existing key
        let re = regex::Regex::new(&format!(r"{}=.*", regex::escape(key))).unwrap();
        *env_content = re
            .replace(env_content, &format!("{}={}", key, value))
            .to_string();
    } else {
        // Add new key
        env_content.push_str(&format!("{}={}\n", key, value));
    }
}
