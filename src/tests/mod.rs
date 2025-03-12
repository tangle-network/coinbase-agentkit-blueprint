use crate::{
    types::{AgentConfig, AgentMode},
    AgentPortConfig, ServiceContext,
};
use blueprint_sdk::config::GadgetConfiguration;
use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

pub mod create_agent_tests;
pub mod deploy_agent_tests;

/// Helper function to set up a temporary test environment
pub fn setup_test_env() -> (ServiceContext, PathBuf) {
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

    // Create an actual .env file for tests in the temp directory root
    let env_file_path = temp_dir.join(".env");
    fs::write(
        &env_file_path,
        "OPENAI_API_KEY=test-api-key\nCDP_API_KEY_NAME=test-cdp-name\nCDP_API_KEY_PRIVATE_KEY=test-cdp-key\nPHALA_CLOUD_API_KEY=mock_api_key\nPHALA_CLOUD_API_ENDPOINT=https://example.com/api\n"
    ).expect("Failed to create .env file");

    // Load the .env file
    dotenv::from_path(&env_file_path).ok();

    // Create a minimal docker-compose.yml file
    fs::write(
        template_dir.join("docker-compose.yml"),
        "version: '3'\nservices:\n  agent:\n    build: .\n    ports:\n      - '3000:3000'\n    environment:\n      - PORT=3000\n      - OPENAI_API_KEY=${OPENAI_API_KEY}\n      - CDP_API_KEY_NAME=${CDP_API_KEY_NAME}\n      - CDP_API_KEY_PRIVATE_KEY=${CDP_API_KEY_PRIVATE_KEY}\n"
    ).expect("Failed to create docker-compose.yml");

    // Create a simple package.json file
    fs::write(
        template_dir.join("package.json"),
        r#"{"name":"agent","version":"1.0.0","main":"index.js","dependencies":{}}"#,
    )
    .expect("Failed to create package.json");

    // Set up mock service context using env vars where possible from .env
    let context = ServiceContext {
        config: GadgetConfiguration::default(),
        call_id: None,
        agents_base_dir: Some(temp_dir.join("agents").to_string_lossy().to_string()),
        tee_enabled: Some(false),
        phala_tee_api_endpoint: Some(
            env::var("PHALA_CLOUD_API_ENDPOINT")
                .unwrap_or_else(|_| "https://cloud-api.phala.network/api/v1".to_string()),
        ),
        phala_tee_api_key: Some(
            env::var("PHALA_CLOUD_API_KEY").unwrap_or_else(|_| "mock_api_key".to_string()),
        ),
        // Initialize the agent ports HashMap
        agent_ports: Some(Arc::new(Mutex::new(HashMap::new()))),
    };

    // Ensure agents directory exists
    fs::create_dir_all(temp_dir.join("agents")).expect("Failed to create agents directory");

    (context, temp_dir)
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
            let vm_config = match deployer.create_vm_config_from_string(
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
            assert!(vm_config.is_object(), "VM config should be a JSON object");
            assert_eq!(vm_config["name"], app_name, "Name should match app_name");

            // Test getting pubkey for the VM config
            let pubkey_response = match deployer.get_pubkey_for_config(&vm_config).await {
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
                pubkey_response.is_object(),
                "Pubkey response should be a JSON object"
            );
            assert!(
                pubkey_response.get("app_env_encrypt_pubkey").is_some(),
                "Response should contain app_env_encrypt_pubkey"
            );
            assert!(
                pubkey_response.get("app_id_salt").is_some(),
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
