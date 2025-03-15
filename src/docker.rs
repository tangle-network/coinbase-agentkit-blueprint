use phala_tee_deploy_rs::{TeeDeployer, TeeDeployerBuilder};
use std::fs;
use std::path::{Path, PathBuf};

/// Creates a Docker Compose file in the agent directory by copying the template
///
/// This function copies the template docker-compose.yml and normalizes it to ensure
/// consistent field ordering for TEE deployment.
///
/// # Arguments
///
/// * `agent_dir` - Path to the agent directory
/// * `agent_id` - Unique identifier for the agent
/// * `http_port` - The HTTP port to expose (default: 3000)
/// * `websocket_port` - The WebSocket port to expose (default: 3001)
/// * `env_vars` - Additional environment variables to include (currently unused)
///
/// # Returns
///
/// The path to the created Docker Compose file
pub fn write_docker_compose_file(agent_dir: &Path) -> Result<PathBuf, String> {
    // Define the source template path
    let template_path = Path::new("templates/starter/docker-compose.yml");
    if !template_path.exists() {
        return Err("Docker Compose template not found".to_string());
    }

    // Read the template
    let docker_compose = fs::read_to_string(template_path)
        .map_err(|e| format!("Failed to read Docker Compose template: {}", e))?;

    // Normalize the Docker Compose file to ensure consistent ordering
    let normalized_compose = normalize_docker_compose(&docker_compose)?;

    // Write the Docker Compose file
    let compose_path = agent_dir.join("docker-compose.yml");
    fs::write(&compose_path, normalized_compose)
        .map_err(|e| format!("Failed to write docker-compose.yml: {}", e))?;

    Ok(compose_path)
}

/// Normalizes a Docker Compose file by parsing it and reserializing it in a consistent format
/// This ensures the same field ordering between different processes
///
/// # Arguments
///
/// * `docker_compose` - The docker-compose content as a string
///
/// # Returns
///
/// A Result containing the normalized Docker Compose content
pub fn normalize_docker_compose(docker_compose: &str) -> Result<String, String> {
    // Parse the Docker Compose content into a structured Value
    let mut yaml: serde_yaml::Value = serde_yaml::from_str(docker_compose)
        .map_err(|e| format!("Failed to parse Docker compose as YAML: {}", e))?;

    // Sort environment variables if they exist to ensure consistent ordering
    if let Some(services) = yaml.get_mut("services") {
        if let Some(agent) = services.get_mut("agent") {
            if let Some(env) = agent.get_mut("environment") {
                if let Some(env_array) = env.as_sequence_mut() {
                    // Sort environment variables by key
                    env_array.sort_by(|a, b| {
                        let a_str = a.as_str().unwrap_or("");
                        let b_str = b.as_str().unwrap_or("");
                        a_str.cmp(b_str)
                    });
                }
            }
        }
    }

    // Convert back to a string in a consistent manner
    serde_yaml::to_string(&yaml).map_err(|e| format!("Failed to serialize normalized YAML: {}", e))
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
