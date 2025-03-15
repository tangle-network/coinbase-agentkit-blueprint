use crate::{
    types::{AgentConfig, AgentMode},
    ServiceContext,
};
use blueprint_sdk::config::GadgetConfiguration;
use dotenv::dotenv;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, path::Path};
use tempfile::tempdir;
use tokio::process::Command as TokioCommand;

pub mod create_agent_tests;
pub mod deploy_agent_tests;

/// Log a message with timestamp for test output
pub fn log(msg: &str) {
    println!("[{}] {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg);
}

/// Clean up any existing containers
async fn clean_existing_container(agent_dir: &Path) -> Result<(), String> {
    log("Cleaning up any existing containers");
    let cleanup_output = TokioCommand::new("docker-compose")
        .args(&["down", "--remove-orphans"])
        .current_dir(agent_dir)
        .output()
        .await;

    if let Ok(output) = &cleanup_output {
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log(&format!("Cleanup warning: {}", stderr));
            // Continue anyway - this is a cleanup operation
        }
    }

    Ok(())
}

/// Helper function to set up a temporary test environment
/// Returns a tuple with (ServiceContext, temporary directory path, Vec of missing requirements)
/// If the Vec is empty, all requirements are met
pub fn setup_test_env() -> (ServiceContext, PathBuf, Vec<String>) {
    // Load .env file
    dotenv().ok();

    let mut missing_requirements = Vec::new();

    // Check for CI environment - tests should be skipped in CI
    if env::var("CI").is_ok() {
        missing_requirements.push("Test running in CI environment".to_string());
    }

    // Check for required environment variables
    let required_vars = [
        "OPENAI_API_KEY",
        "CDP_API_KEY_NAME",
        "CDP_API_KEY_PRIVATE_KEY",
    ];
    for var in required_vars {
        if env::var(var).is_err() {
            missing_requirements.push(format!("Missing environment variable: {}", var));
        }
    }

    // Check Docker availability for deployment tests
    let docker_available = std::process::Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if !docker_available {
        missing_requirements.push("Docker is not available".to_string());
    }

    // Create a temporary directory for the test
    let temp_dir = tempdir()
        .expect("Failed to create temp directory")
        .into_path();

    // Create template directories with starter files
    let template_dir = temp_dir.join("templates/starter");
    fs::create_dir_all(&template_dir).expect("Failed to create template directory");

    // Create a minimal example .env file
    fs::write(
        template_dir.join(".env.example"),
        "OPENAI_API_KEY=your_openai_api_key_here\nAGENT_MODE=cli-chat\n# MODEL=gpt-4o-mini\nAGENT_PORT=3000\n"
    ).expect("Failed to create .env.example");

    // Create a minimal docker-compose.yml file
    fs::write(
        template_dir.join("docker-compose.yml"),
        "version: '3'\nservices:\n  agent:\n    build: .\n    ports:\n      - '3000:3000'\n    environment:\n      - PORT=3000\n      - OPENAI_API_KEY=${OPENAI_API_KEY}\n      - CDP_API_KEY_NAME=${CDP_API_KEY_NAME}\n      - CDP_API_KEY_PRIVATE_KEY=${CDP_API_KEY_PRIVATE_KEY}\n"
    ).expect("Failed to create docker-compose.yml");

    // Create dummy files needed for the tests
    fs::write(
        template_dir.join("Dockerfile"),
        "FROM node:18\nWORKDIR /app\nCOPY . .\nCMD [\"echo\", \"Mock container\"]\n",
    )
    .expect("Failed to create Dockerfile");

    // Create an agent port map
    let agent_ports = Arc::new(Mutex::new(HashMap::new()));

    // Create a minimal service context
    let context = ServiceContext {
        config: GadgetConfiguration::default(),
        call_id: None,
        agent_ports: Some(agent_ports),
        agents_base_dir: Some(temp_dir.to_string_lossy().to_string()),
        tee_enabled: Some(false),
        phala_tee_api_key: Some("mock_api_key".to_string()),
        phala_tee_api_endpoint: Some("https://example.com/api".to_string()),
    };

    (context, temp_dir, missing_requirements)
}

#[test]
fn test_agent_config() {
    let config = AgentConfig {
        mode: AgentMode::Autonomous,
        model: "gpt-4o-mini".to_string(),
    };

    assert!(matches!(config.mode, AgentMode::Autonomous));
}

/// Test creating a VM configuration for TEE deployment
#[tokio::test]
async fn test_vm_config_creation() {
    // Load environment variables
    dotenv().ok();

    // Create a test Docker Compose content with proper structure
    let docker_compose = r#"
version: '3'
services:
  agent:
    build:
      context: .
    image: "phala/phala-sgx_2.17.1:latest"
    ports:
      - "3000:3000"
      - "3001:3001"
    environment:
      - PORT=3000
      - WEBSOCKET_PORT=3001
"#;

    // Initialize a TeeDeployer with test credentials
    let api_key = env::var("PHALA_CLOUD_API_KEY").unwrap_or_else(|_| "mock_api_key".to_string());
    let api_endpoint = env::var("PHALA_CLOUD_API_ENDPOINT")
        .unwrap_or_else(|_| "https://cloud-api.phala.network/api/v1".to_string());

    println!("Using Phala API endpoint: {}", api_endpoint);

    let mut deployer = match phala_tee_deploy_rs::TeeDeployerBuilder::new()
        .with_api_key(api_key)
        .with_api_endpoint(api_endpoint)
        .build()
    {
        Ok(d) => d,
        Err(e) => {
            println!("Skipping test - TeeDeployer couldn't be initialized: {}", e);
            return;
        }
    };

    // Discover TEEPods - this automatically selects one for use
    println!("Attempting to discover TEEPods...");
    match deployer.discover_teepod().await {
        Ok(_) => {
            println!("Successfully discovered and selected a TEEPod");

            // Create VM configuration using the deployer with the auto-selected TEEPod
            let app_name = "test-coinbase-agent";
            let vm_config = match deployer.create_vm_config(
                docker_compose,
                app_name,
                Some(2),
                Some(2048),
                Some(10),
            ) {
                Ok(config) => {
                    println!("Successfully created VM config");
                    config
                }
                Err(e) => {
                    panic!("Failed to create VM config after TEEPod discovery: {}", e);
                }
            };

            // Verify the VM config structure
            assert_eq!(vm_config.name, app_name, "Name should match app_name");

            // Test getting pubkey for the VM config
            let vm_config_json = serde_json::to_value(&vm_config).unwrap();
            let pubkey_response = match deployer.get_pubkey_for_config(&vm_config_json).await {
                Ok(response) => {
                    println!("Successfully retrieved pubkey");
                    response
                }
                Err(e) => {
                    panic!(
                        "Failed to get pubkey after successful VM config creation: {}",
                        e
                    );
                }
            };

            // Verify pubkey response structure
            assert!(
                pubkey_response.app_env_encrypt_pubkey.len() > 0,
                "Response should contain app_env_encrypt_pubkey"
            );
            assert!(
                pubkey_response.app_id_salt.len() > 0,
                "Response should contain app_id_salt"
            );
        }
        Err(e) => {
            panic!(
                "Failed to get pubkey after successful VM config creation: {}",
                e
            );
        }
    }
}
