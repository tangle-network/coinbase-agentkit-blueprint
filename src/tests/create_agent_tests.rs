use crate::{
    create_agent::handle_create_agent,
    tests::setup_test_env,
    types::{
        AgentConfig, AgentCreationResult, AgentMode, ApiKeyConfig, CreateAgentParams,
        DeploymentConfig,
    },
};
use dotenv::dotenv;
use std::env;

/// Test agent creation without TEE
#[tokio::test]
async fn test_create_agent_no_tee() {
    // Skip test if CI environment is detected
    if env::var("CI").is_ok() {
        println!("Skipping test in CI environment");
        return;
    }

    // Load dotenv from the current directory for the test
    dotenv().ok();

    let (context, _temp_dir) = setup_test_env();

    // Create agent parameters
    let params = CreateAgentParams {
        name: "Test Agent".to_string(),
        agent_config: AgentConfig {
            mode: AgentMode::Autonomous,
            model: "gpt-4o-mini".to_string(),
        },
        deployment_config: DeploymentConfig {
            tee_enabled: false,
            docker_compose_path: None,
            public_key: None,
            http_port: Some(3000),
            tee_config: None,
        },
        api_key_config: ApiKeyConfig {
            openai_api_key: Some(
                env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test-api-key".to_string()),
            ),
        },
    };

    // Serialize params
    let params_bytes = serde_json::to_vec(&params).expect("Failed to serialize params");

    // Call the handle_create_agent function
    let result = handle_create_agent(params_bytes, &context).await;

    // Verify the result
    assert!(result.is_ok(), "Agent creation failed: {:?}", result.err());

    // Deserialize the result to check details
    let result_bytes = result.unwrap();
    let result: AgentCreationResult =
        serde_json::from_slice(&result_bytes).expect("Failed to deserialize result");

    // Assertions
    assert!(!result.agent_id.is_empty(), "Agent ID should not be empty");
    assert_eq!(result.files_created.len(), 3, "Should have created 3 files");
    assert!(
        result.tee_public_key.is_none(),
        "TEE public key should be None"
    );
}

/// Test agent creation with TEE enabled
#[tokio::test]
async fn test_create_agent_with_tee() {
    // Skip test if CI environment is detected
    if env::var("CI").is_ok() {
        println!("Skipping test in CI environment");
        return;
    }

    // Load dotenv from the current directory for the test
    dotenv().ok();

    let mut context = setup_test_env().0;

    // Enable TEE and set required config
    context.tee_enabled = Some(true);
    context.phala_tee_api_key =
        Some(env::var("PHALA_CLOUD_API_KEY").unwrap_or_else(|_| "test-tee-key".to_string()));
    context.phala_tee_api_endpoint = Some(
        env::var("PHALA_CLOUD_API_ENDPOINT")
            .unwrap_or_else(|_| "https://cloud-api.phala.network/api/v1".to_string()),
    );

    // Create agent parameters with TEE enabled
    let params = CreateAgentParams {
        name: "Test TEE Agent".to_string(),
        agent_config: AgentConfig {
            mode: AgentMode::Autonomous,
            model: "gpt-4o-mini".to_string(),
        },
        deployment_config: DeploymentConfig {
            tee_enabled: true,
            docker_compose_path: None,
            public_key: None,
            http_port: Some(3000),
            tee_config: None,
        },
        api_key_config: ApiKeyConfig {
            openai_api_key: Some(
                env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test-api-key".to_string()),
            ),
        },
    };

    // Serialize params
    let params_bytes = serde_json::to_vec(&params).expect("Failed to serialize params");

    // Call the handle_create_agent function
    let result = handle_create_agent(params_bytes, &context).await;

    // Verify the result
    assert!(result.is_ok(), "Agent creation failed: {:?}", result.err());

    // Deserialize the result to check details
    let result_bytes = result.unwrap();
    let result: AgentCreationResult =
        serde_json::from_slice(&result_bytes).expect("Failed to deserialize result");

    // Assertions
    assert!(!result.agent_id.is_empty(), "Agent ID should not be empty");
    assert_eq!(result.files_created.len(), 3, "Should have created 3 files");
    assert!(
        result.tee_public_key.is_some(),
        "TEE public key should be present"
    );
    assert!(
        !result.tee_public_key.unwrap().is_empty(),
        "TEE public key should not be empty"
    );
}
