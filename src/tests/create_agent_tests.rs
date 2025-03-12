use crate::{
    create_agent::handle_create_agent,
    tests::{log, setup_test_env},
    types::{
        AgentConfig, AgentCreationResult, AgentMode, ApiKeyConfig, CreateAgentParams,
        DeploymentConfig,
    },
};
use std::env;

/// Test agent creation without TEE
#[tokio::test]
async fn test_create_agent_no_tee() {
    // Set up test environment and check requirements
    let (context, _temp_dir, missing) = setup_test_env();

    // Skip test if requirements not met
    if !missing.is_empty() {
        for issue in missing {
            log(&format!("Skipping test: {}", issue));
        }
        return;
    }

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
            openai_api_key: Some(env::var("OPENAI_API_KEY").unwrap()),
            cdp_api_key_name: Some(env::var("CDP_API_KEY_NAME").unwrap()),
            cdp_api_key_private_key: Some(env::var("CDP_API_KEY_PRIVATE_KEY").unwrap()),
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
    // Set up test environment and check requirements
    let (mut context, _temp_dir, missing) = setup_test_env();

    // Skip test if requirements not met
    if !missing.is_empty() {
        for issue in missing {
            log(&format!("Skipping test: {}", issue));
        }
        return;
    }

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
            openai_api_key: Some(env::var("OPENAI_API_KEY").unwrap()),
            cdp_api_key_name: Some(env::var("CDP_API_KEY_NAME").unwrap()),
            cdp_api_key_private_key: Some(env::var("CDP_API_KEY_PRIVATE_KEY").unwrap()),
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
