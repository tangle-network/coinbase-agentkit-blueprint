use crate::types::{AgentCreationResult, CreateAgentParams, TeeConfig};
use crate::ServiceContext;
use blueprint_sdk::logging;
use phala_tee_deploy_rs::{DeploymentConfig as TeeDeployConfig, TeeClient};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Handles the create_agent job
pub async fn handle_create_agent(
    params_bytes: Vec<u8>,
    context: &ServiceContext,
) -> Result<Vec<u8>, String> {
    // Deserialize the parameters from bytes
    let params: CreateAgentParams = match serde_json::from_slice(&params_bytes) {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to deserialize parameters: {}", e)),
    };

    // Generate a unique ID for this agent
    let agent_id = Uuid::new_v4().to_string();
    logging::info!("Creating agent with ID: {}", agent_id);

    // Create the agent directory and copy starter template
    let agent_dir = setup_agent_directory(&agent_id, context)?;
    logging::info!("Created agent directory: {}", agent_dir.display());

    // Create .env file with configuration
    create_env_file(&params, &agent_dir)?;
    logging::info!("Created environment configuration");

    // Get TEE public key if TEE is enabled
    let tee_config = if params.deployment_config.tee_enabled {
        get_tee_public_key(context).await?
    } else {
        None
    };

    // Return the result
    let result = AgentCreationResult {
        agent_id,
        files_created: vec![
            agent_dir.join(".env").to_string_lossy().to_string(),
            agent_dir.join("package.json").to_string_lossy().to_string(),
            agent_dir
                .join("docker-compose.yml")
                .to_string_lossy()
                .to_string(),
        ],
        tee_public_key: tee_config.as_ref().and_then(|c| c.pubkey.clone()),
        tee_pubkey: tee_config.as_ref().and_then(|c| c.pubkey.clone()),
    };

    // Serialize the result
    match serde_json::to_vec(&result) {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(format!("Failed to serialize result: {}", e)),
    }
}

/// Sets up the agent directory by copying the starter template
fn setup_agent_directory(agent_id: &str, context: &ServiceContext) -> Result<PathBuf, String> {
    // Define base directory from context or environment
    let base_dir = context
        .get_env_var("AGENT_BASE_DIR")
        .unwrap_or_else(|| "./agents".to_string());

    // Create the base directory if it doesn't exist
    fs::create_dir_all(&base_dir).map_err(|e| format!("Failed to create base directory: {}", e))?;

    // Create a directory for this agent
    let agent_dir = PathBuf::from(&base_dir).join(agent_id);
    fs::create_dir(&agent_dir).map_err(|e| format!("Failed to create agent directory: {}", e))?;

    // Copy starter template
    copy_starter_template(&agent_dir)?;

    Ok(agent_dir)
}

/// Copies the starter template to the agent directory
fn copy_starter_template(agent_dir: &Path) -> Result<(), String> {
    let template_dir = PathBuf::from("templates/starter");
    if !template_dir.exists() {
        return Err("Starter template directory not found".to_string());
    }

    // Use fs::read_dir and recursively copy files instead of shell command
    copy_dir_contents(&template_dir, agent_dir)
}

/// Recursively copy directory contents
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("{} is not a directory", src.display()));
    }

    // Read the source directory entries
    let entries = match fs::read_dir(src) {
        Ok(entries) => entries,
        Err(e) => return Err(format!("Failed to read directory {}: {}", src.display(), e)),
    };

    // Process each entry
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => return Err(format!("Failed to read directory entry: {}", e)),
        };

        let src_path = entry.path();
        let file_name = match src_path.file_name() {
            Some(name) => name,
            None => continue, // Skip entries without a valid file name
        };

        // Skip node_modules directory to avoid copying large dependency trees
        if file_name == "node_modules" || file_name == ".yarn" {
            continue;
        }

        let dst_path = dst.join(file_name);

        if src_path.is_dir() {
            // Create the destination directory
            fs::create_dir_all(&dst_path)
                .map_err(|e| format!("Failed to create directory {}: {}", dst_path.display(), e))?;

            // Recursively copy contents
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            // Copy the file
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "Failed to copy {} to {}: {}",
                    src_path.display(),
                    dst_path.display(),
                    e
                )
            })?;
        }
    }

    Ok(())
}

/// Get TEE public key for environment variable encryption
async fn get_tee_public_key(context: &ServiceContext) -> Result<Option<TeeConfig>, String> {
    let tee_api_key = match context.get_env_var("PHALA_CLOUD_API_KEY") {
        Some(key) => key,
        None => return Ok(None),
    };

    let teepod_id = match context.get_env_var("PHALA_TEEPOD_ID") {
        Some(id) => id
            .parse::<u64>()
            .map_err(|e| format!("Invalid PHALA_TEEPOD_ID: {}", e))?,
        None => return Ok(None),
    };

    let tee_api_endpoint = context
        .get_env_var("PHALA_CLOUD_API_ENDPOINT")
        .unwrap_or_else(|| "https://cloud-api.phala.network/api/v1".to_string());

    // Initialize TEE client with minimal config
    let tee_config = TeeDeployConfig::new(
        tee_api_key.clone(),
        String::new(), // Empty docker compose since we don't need it yet
        HashMap::new(),
        teepod_id,
        "phala-worker:latest".to_string(),
    );

    let client = TeeClient::new(tee_config)
        .map_err(|e| format!("Failed to initialize TEE client: {}", e))?;

    // Get encryption key for environment variables
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
        .ok_or_else(|| "Invalid public key response".to_string())?
        .to_string();

    Ok(Some(TeeConfig {
        enabled: true,
        api_key: Some(tee_api_key),
        api_endpoint: Some(tee_api_endpoint),
        teepod_id: Some(teepod_id),
        app_id: None,
        pubkey: Some(pubkey),
        encrypted_env: None,
    }))
}

/// Creates a .env file with the necessary environment variables
fn create_env_file(params: &CreateAgentParams, agent_dir: &Path) -> Result<(), String> {
    let env_file_path = agent_dir.join(".env");
    let env_template_path = agent_dir.join(".env.example");

    // Read the template
    let template = fs::read_to_string(&env_template_path)
        .map_err(|e| format!("Failed to read .env.example: {}", e))?;

    // Create new content with actual values
    let mut env_content = template.clone();

    // Replace OpenAI API key if provided
    if let Some(api_key) = &params.api_key_config.openai_api_key {
        env_content = env_content.replace(
            "OPENAI_API_KEY=your_openai_api_key_here",
            &format!("OPENAI_API_KEY={}", api_key),
        );
    }

    // Set agent mode
    env_content = env_content.replace(
        "AGENT_MODE=cli-chat",
        &format!(
            "AGENT_MODE={}",
            params.agent_config.mode.to_string().to_lowercase()
        ),
    );

    // Set model name
    env_content = env_content.replace(
        "# MODEL=gpt-4o-mini",
        &format!("MODEL={}", params.agent_config.model),
    );

    // Add HTTP port if provided
    if let Some(port) = params.deployment_config.http_port {
        env_content = env_content.replace("AGENT_PORT=3000", &format!("AGENT_PORT={}", port));
    }

    // Write the .env file
    fs::write(&env_file_path, env_content)
        .map_err(|e| format!("Failed to write .env file: {}", e))?;

    Ok(())
}
