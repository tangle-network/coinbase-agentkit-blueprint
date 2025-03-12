use crate::docker;
use crate::types::{AgentCreationResult, CreateAgentParams, TeeConfig};
use crate::{AgentPortConfig, ServiceContext};
use blueprint_sdk::logging;
use dockworker::ComposeConfig;
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

    // Get HTTP port from params or use default 3000
    let http_port = params.deployment_config.http_port.unwrap_or(3000);
    let websocket_port = http_port + 1;

    // Store port configuration in the context for later use during deployment
    if let Some(agent_ports) = &context.agent_ports {
        if let Ok(mut ports_map) = agent_ports.lock() {
            ports_map.insert(
                agent_id.clone(),
                AgentPortConfig {
                    http_port,
                    websocket_port,
                },
            );
            logging::info!(
                "Registered agent {} with ports HTTP:{}, WS:{}",
                agent_id,
                http_port,
                websocket_port
            );
        } else {
            logging::warn!("Failed to lock agent_ports map for agent {}", agent_id);
        }
    } else {
        logging::warn!("No agent_ports map available in context");
    }

    // Create Docker Compose file
    let env_vars = HashMap::new(); // No additional env vars at creation time
    let compose_path = docker::write_docker_compose_file(
        &agent_dir,
        &agent_id,
        Some(http_port),
        Some(websocket_port),
        env_vars,
    )?;

    // Prepare TEE config if enabled
    let tee_config = if params.deployment_config.tee_enabled {
        get_tee_public_key(&agent_dir, context).await?
    } else {
        None
    };

    // Return the result
    let result = AgentCreationResult {
        agent_id,
        files_created: vec![
            agent_dir.join(".env").to_string_lossy().to_string(),
            agent_dir.join("package.json").to_string_lossy().to_string(),
            compose_path.to_string_lossy().to_string(),
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
    // Define base directory directly from context
    let base_dir = match &context.agents_base_dir {
        Some(dir) => dir.clone(),
        None => "./agents".to_string(),
    };

    // Create the base directory if it doesn't exist
    fs::create_dir_all(&base_dir).map_err(|e| format!("Failed to create base directory: {}", e))?;

    // Create a directory for this agent
    let agent_dir = PathBuf::from(&base_dir).join(agent_id);
    fs::create_dir(&agent_dir).map_err(|e| format!("Failed to create agent directory: {}", e))?;

    // Copy starter template
    copy_starter_template(&agent_dir)?;

    Ok(agent_dir)
}

/// Copies the starter template to the agent directory and installs dependencies
fn copy_starter_template(agent_dir: &Path) -> Result<(), String> {
    let template_dir = PathBuf::from("templates/starter");
    if !template_dir.exists() {
        return Err("Starter template directory not found".to_string());
    }

    // Copy all files from the template directory to the agent directory
    copy_dir_contents(&template_dir, agent_dir)?;

    // Run yarn install in the agent directory to set up node_modules
    logging::info!(
        "Installing dependencies in agent directory: {}",
        agent_dir.display()
    );
    // Try with Yarn 4 syntax
    match std::process::Command::new("yarn")
        .args(&["install"])
        .current_dir(agent_dir)
        .status()
    {
        Ok(status) if status.success() => {
            logging::info!("Successfully installed dependencies");
            Ok(())
        }
        Ok(_) => {
            logging::warn!("Failed to install dependencies with yarn, but continuing");

            // As a fallback, try with npm
            logging::info!("Trying npm install as fallback");
            match std::process::Command::new("npm")
                .args(&["install"])
                .current_dir(agent_dir)
                .status()
            {
                Ok(status) if status.success() => {
                    logging::info!("Successfully installed dependencies with npm");
                }
                Ok(_) => {
                    logging::warn!("Failed to install dependencies with npm, but continuing");
                }
                Err(e) => {
                    logging::warn!("Error running npm install: {}", e);
                }
            }

            Ok(()) // Continue even if install fails
        }
        Err(e) => {
            logging::warn!("Error running yarn install: {}", e);
            Ok(()) // Continue even if yarn install fails
        }
    }
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

/// Get TEE public key for environment variable encryption using TeeDeployer
async fn get_tee_public_key(
    agent_dir: &Path,
    context: &ServiceContext,
) -> Result<Option<TeeConfig>, String> {
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

    logging::info!("Initializing TeeDeployer for public key retrieval");

    // Initialize the TeeDeployer
    let mut deployer = docker::init_tee_deployer(tee_api_key, tee_api_endpoint)?;

    // Discover an available TEEPod
    logging::info!("Discovering available TEEPods...");
    deployer
        .discover_teepod()
        .await
        .map_err(|e| format!("Failed to discover TEEPods: {}", e))?;

    // Read docker-compose.yml from the agent directory
    let docker_compose_path = agent_dir.join("docker-compose.yml");
    let docker_compose = fs::read_to_string(&docker_compose_path)
        .map_err(|e| format!("Failed to read docker-compose.yml: {}", e))?;

    // Create VM configuration using TeeDeployer's native method
    logging::info!("Creating VM configuration from Docker Compose");

    // Parse docker-compose.yml to ComposeConfig using dockworker
    let compose_config: ComposeConfig = serde_yaml::from_str(&docker_compose)
        .map_err(|e| format!("Failed to parse docker-compose.yml: {}", e))?;

    // Use TeeDeployer's built-in create_vm_config method
    let app_name = format!(
        "coinbase-agent-{}",
        agent_dir.file_name().unwrap().to_string_lossy()
    );
    let vm_config = deployer
        .create_vm_config(
            &compose_config,
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
        .ok_or_else(|| "Missing public key in response".to_string())?
        .to_string();

    let salt = pubkey_response["app_id_salt"]
        .as_str()
        .ok_or_else(|| "Missing salt in response".to_string())?
        .to_string();

    logging::info!("Successfully obtained TEE public key");

    Ok(Some(TeeConfig {
        enabled: true,
        api_key: Some(tee_api_key.clone()),
        api_endpoint: Some(tee_api_endpoint.clone()),
        app_id: Some(salt),
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
