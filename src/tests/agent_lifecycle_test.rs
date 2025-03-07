use crate::create_agent::handle_create_agent;
use crate::deploy_agent::handle_deploy_agent;
use crate::types::{
    AgentConfig, AgentCreationResult, AgentDeploymentResult, AgentMode, ApiKeyConfig,
    CreateAgentParams, DeployAgentParams, DeploymentConfig,
};
use crate::ServiceContext;
use serde_json;
use std::env;
use std::path::Path;
use tempfile::TempDir;

/// Integration test for the full agent lifecycle without TEE
#[tokio::test]
async fn test_agent_lifecycle_no_tee() {
    println!("Starting agent lifecycle integration test (no TEE)...");

    // Set up test environment with a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let test_dir = temp_dir.path();
    println!("Test directory: {}", test_dir.display());

    // Set environment variables needed for testing
    let env_vars = setup_environment_variables(test_dir, false);

    // Create service context
    let context = create_service_context(&env_vars);

    // 1. Create an agent using the actual handler
    let (agent_id, agent_result) = create_agent(&context).await;
    println!("Agent created with ID: {}", agent_id);

    // Verify agent was created correctly
    verify_agent_created(test_dir, &agent_id, &agent_result).await;

    // 2. Deploy the agent using the actual handler
    let deployment_result = deploy_agent(&agent_id, None, &context).await;
    println!(
        "Agent deployed with ID: {}",
        deployment_result.deployment_id
    );

    // Verify agent is running
    verify_agent_deployed(&agent_id).await;

    // 3. Clean up resources
    clean_up(&agent_id).await;

    println!("Agent lifecycle test (no TEE) completed successfully");
}

/// Integration test for the full agent lifecycle with TEE
#[tokio::test]
async fn test_agent_lifecycle_tee() {
    println!("Starting agent lifecycle integration test (TEE)...");

    // Set up test environment with a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let test_dir = temp_dir.path();
    println!("Test directory: {}", test_dir.display());

    // Set environment variables needed for testing
    let env_vars = setup_environment_variables(test_dir, true);

    // Create service context
    let context = create_service_context(&env_vars);

    // 1. Create an agent using the actual handler
    let (agent_id, agent_result) = create_agent(&context).await;
    println!("Agent created with ID: {}", agent_id);

    // Verify agent was created correctly and got TEE public key
    verify_agent_created(test_dir, &agent_id, &agent_result).await;
    assert!(
        agent_result.tee_pubkey.is_some(),
        "TEE public key not received"
    );

    // 2. Simulate encrypting environment variables with the TEE public key
    let encrypted_env = simulate_env_encryption(&agent_result.tee_pubkey.unwrap());

    // 3. Deploy the agent using the actual handler
    let deployment_result = deploy_agent(&agent_id, Some(encrypted_env), &context).await;
    println!(
        "Agent deployed with ID: {}",
        deployment_result.deployment_id
    );

    // Verify TEE deployment
    assert!(
        deployment_result.tee_app_id.is_some(),
        "TEE app ID not received"
    );

    // 4. Clean up resources
    clean_up(&agent_id).await;

    println!("Agent lifecycle test (TEE) completed successfully");
}

/// Setup environment variables for testing
fn setup_environment_variables(test_dir: &Path, tee_enabled: bool) -> Vec<(String, String)> {
    let mut env_vars = vec![
        (
            "AGENT_BASE_DIR".to_string(),
            test_dir.to_string_lossy().to_string(),
        ),
        ("HTTP_PORT".to_string(), "3000".to_string()),
        ("SERVER_HOST".to_string(), "localhost".to_string()),
    ];

    if tee_enabled {
        env_vars.extend(vec![
            ("TEE_ENABLED".to_string(), "true".to_string()),
            (
                "PHALA_CLOUD_API_KEY".to_string(),
                "test-phala-api-key".to_string(),
            ),
            (
                "PHALA_CLOUD_API_ENDPOINT".to_string(),
                "https://test-api.phala.network".to_string(),
            ),
            ("PHALA_TEEPOD_ID".to_string(), "12345".to_string()),
        ]);
    } else {
        env_vars.push(("TEE_ENABLED".to_string(), "false".to_string()));
    }

    // Apply environment variables
    for (key, value) in &env_vars {
        env::set_var(key, value);
    }

    env_vars
}

/// Create a service context for testing
fn create_service_context(env_vars: &[(String, String)]) -> ServiceContext {
    ServiceContext {
        config: Default::default(),
        call_id: None,
        agent_base_dir: env_vars
            .iter()
            .find(|(k, _)| k == "AGENT_BASE_DIR")
            .map(|(_, v)| v.clone()),
        tee_enabled: env_vars
            .iter()
            .find(|(k, _)| k == "TEE_ENABLED")
            .map(|(_, v)| v == "true"),
        tee_provider: None,
        tee_api_key: None,
    }
}

/// Create an agent using the actual handler
async fn create_agent(context: &ServiceContext) -> (String, AgentCreationResult) {
    println!("Creating agent using handler_create_agent...");

    // Define test agent parameters
    let create_params = CreateAgentParams {
        name: "Test Agent".to_string(),
        agent_config: AgentConfig {
            mode: AgentMode::Chat,
            model: "gpt-4o-mini".to_string(),
        },
        deployment_config: DeploymentConfig {
            tee_enabled: context.tee_enabled.unwrap_or(false),
            docker_compose_path: None,
            public_key: None,
            http_port: Some(3000),
            tee_config: None,
        },
        api_key_config: ApiKeyConfig {
            openai_api_key: Some("test-openai-api-key".to_string()),
        },
    };

    // Serialize parameters
    let params_bytes =
        serde_json::to_vec(&create_params).expect("Failed to serialize creation parameters");

    // Call the handler
    let result_bytes = handle_create_agent(params_bytes, context)
        .await
        .expect("Failed to create agent");

    // Parse result
    let result: AgentCreationResult =
        serde_json::from_slice(&result_bytes).expect("Failed to deserialize creation result");

    (result.agent_id.clone(), result)
}

/// Verify the agent was created correctly
async fn verify_agent_created(
    test_dir: &Path,
    agent_id: &str,
    creation_result: &AgentCreationResult,
) {
    println!("Verifying agent creation...");

    // Check agent directory exists
    let agent_dir = Path::new(test_dir).join(agent_id);
    assert!(
        agent_dir.exists(),
        "Agent directory doesn't exist: {}",
        agent_dir.display()
    );

    // Check all files reported as created actually exist
    for file in &creation_result.files_created {
        let file_path = Path::new(file);
        assert!(file_path.exists(), "Created file doesn't exist: {}", file);
    }

    // Check key files are in place
    let required_files = ["package.json", "docker-compose.yml", ".env"];
    for file in &required_files {
        let file_path = agent_dir.join(file);
        assert!(
            file_path.exists(),
            "Required file missing: {}",
            file_path.display()
        );
    }

    println!("Agent creation verified successfully");
}

/// Deploy an agent using the actual handler
async fn deploy_agent(
    agent_id: &str,
    encrypted_env: Option<String>,
    context: &ServiceContext,
) -> AgentDeploymentResult {
    println!("Deploying agent using handle_deploy_agent...");

    // Define deployment parameters
    let deploy_params = DeployAgentParams {
        agent_id: agent_id.to_string(),
        api_key_config: None,
        encrypted_env_vars: encrypted_env,
    };

    // Serialize parameters
    let params_bytes =
        serde_json::to_vec(&deploy_params).expect("Failed to serialize deployment parameters");

    // Call the handler
    let result_bytes = handle_deploy_agent(params_bytes, context)
        .await
        .expect("Failed to deploy agent");

    // Parse result
    let result: AgentDeploymentResult =
        serde_json::from_slice(&result_bytes).expect("Failed to deserialize deployment result");

    println!("Agent deployed with result: {:?}", result);
    result
}

/// Verify the agent is running
async fn verify_agent_deployed(agent_id: &str) {
    println!("Verifying agent deployment...");

    // For TEE deployments, we would verify the deployment status through the Phala API
    // For local deployments, we check if the container is running
    let output = tokio::process::Command::new("docker")
        .args(&[
            "ps",
            "--filter",
            &format!("name=coinbase-agent-{}", agent_id),
        ])
        .output()
        .await
        .expect("Failed to execute docker ps");

    assert!(
        output.status.success(),
        "Failed to check container status: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_str = String::from_utf8_lossy(&output.stdout);
    assert!(
        output_str.contains(&format!("coinbase-agent-{}", agent_id)),
        "Container not found in running containers"
    );

    println!("Agent deployment verified successfully");
}

/// Simulate encrypting environment variables with TEE public key
fn simulate_env_encryption(pubkey: &str) -> String {
    // In a real implementation, this would use the TEE public key to encrypt
    // the environment variables. For testing, we'll just return a mock value.
    format!("encrypted-env-vars-with-key-{}", pubkey)
}

/// Clean up resources
async fn clean_up(agent_id: &str) {
    println!("Cleaning up resources...");

    // Stop and remove the container
    let _ = tokio::process::Command::new("docker")
        .args(&["rm", "-f", &format!("coinbase-agent-{}", agent_id)])
        .output()
        .await;

    println!("Cleanup completed");
}
