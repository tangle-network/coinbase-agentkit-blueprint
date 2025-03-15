use crate::{
    agent_endpoint::AgentEndpoint,
    create_agent::handle_create_agent,
    deploy_agent::handle_deploy_agent,
    tests::{clean_existing_container, log, setup_test_env},
    types::{
        AgentConfig, AgentCreationResult, AgentDeploymentResult, AgentMode, ApiKeyConfig,
        CreateAgentParams, DeployAgentParams, DeploymentConfig,
    },
};
use phala_tee_deploy_rs::Encryptor;
use rand;
use std::{
    env,
    path::Path,
    time::{Duration, Instant},
};

/// Test agent deployment without TEE
#[tokio::test]
async fn test_deploy_agent_local() {
    // Set up test environment and check requirements
    let (context, _temp_dir, missing) = setup_test_env();

    // Skip test if requirements not met
    if !missing.is_empty() {
        for issue in missing {
            log(&format!("Skipping test: {}", issue));
        }
        return;
    }

    // Create an agent
    let create_params = CreateAgentParams {
        name: "Test Agent".to_string(),
        agent_config: AgentConfig {
            mode: AgentMode::Chat,
            model: "gpt-4o-mini".to_string(),
        },
        deployment_config: DeploymentConfig {
            tee_enabled: false,
            docker_compose_path: None,
            http_port: Some(3000),
        },
        api_key_config: ApiKeyConfig {
            openai_api_key: Some(env::var("OPENAI_API_KEY").unwrap()),
            cdp_api_key_name: Some(env::var("CDP_API_KEY_NAME").unwrap()),
            cdp_api_key_private_key: Some(env::var("CDP_API_KEY_PRIVATE_KEY").unwrap()),
        },
    };

    let create_params_bytes =
        serde_json::to_vec(&create_params).expect("Failed to serialize create params");
    let create_result_bytes = handle_create_agent(create_params_bytes, &context)
        .await
        .expect("Agent creation failed");

    let create_result: AgentCreationResult =
        serde_json::from_slice(&create_result_bytes).expect("Failed to deserialize create result");

    // Deploy the agent (expected to fail in test environment without Docker)
    let deploy_params = DeployAgentParams {
        agent_id: create_result.agent_id,
        api_key_config: Some(ApiKeyConfig {
            openai_api_key: Some(env::var("OPENAI_API_KEY").unwrap()),
            cdp_api_key_name: Some(env::var("CDP_API_KEY_NAME").unwrap()),
            cdp_api_key_private_key: Some(env::var("CDP_API_KEY_PRIVATE_KEY").unwrap()),
        }),
        encrypted_env: None,
    };

    let deploy_params_bytes =
        serde_json::to_vec(&deploy_params).expect("Failed to serialize deploy params");
    let deploy_result = handle_deploy_agent(deploy_params_bytes, &context).await;

    // The deployment should fail with a Docker-related error
    match deploy_result {
        Ok(_) => log("Deployment succeeded unexpectedly - Docker must be available"),
        Err(e) => {
            log(&format!("Expected deployment error: {}", e));
            assert!(
                e.contains("docker-compose") || e.contains("Docker") || e.contains("container"),
                "Error should be related to Docker, got: {}",
                e
            );
        }
    }
}

/// Test agent deployment and interaction with the deployed agent
#[tokio::test]
async fn test_deploy_agent_interaction() {
    let start_time = Instant::now();

    // Set up test environment and check requirements
    let (context, _temp_dir, missing) = setup_test_env();

    // Skip test if requirements not met
    if !missing.is_empty() {
        for issue in missing {
            log(&format!("Skipping test: {}", issue));
        }
        return;
    }

    log("Starting deploy agent interaction test");

    // Get API keys from environment
    let openai_api_key = env::var("OPENAI_API_KEY").unwrap();
    let cdp_api_key_name = env::var("CDP_API_KEY_NAME").unwrap();
    let cdp_api_key_private_key = env::var("CDP_API_KEY_PRIVATE_KEY").unwrap();

    // Log credentials (partially masked)
    log("Using CDP credentials from .env file");

    // Use random ports to avoid conflicts
    let http_port = 10000 + (rand::random::<u16>() % 1000);
    let websocket_port = http_port + 1;
    log(&format!(
        "Using ports: HTTP={}, WebSocket={}",
        http_port, websocket_port
    ));

    // Create agent
    log("Creating agent");
    let create_params = CreateAgentParams {
        name: "Interactive Test Agent".to_string(),
        agent_config: AgentConfig {
            mode: AgentMode::Chat,
            model: "gpt-4o-mini".to_string(),
        },
        deployment_config: DeploymentConfig {
            tee_enabled: false,
            docker_compose_path: None,
            http_port: Some(http_port),
        },
        api_key_config: ApiKeyConfig {
            openai_api_key: Some(openai_api_key.clone()),
            cdp_api_key_name: Some(cdp_api_key_name.clone()),
            cdp_api_key_private_key: Some(cdp_api_key_private_key.clone()),
        },
    };

    let create_params_bytes =
        serde_json::to_vec(&create_params).expect("Failed to serialize create params");
    let create_result = handle_create_agent(create_params_bytes, &context)
        .await
        .expect("Agent creation failed");

    let create_result: AgentCreationResult =
        serde_json::from_slice(&create_result).expect("Failed to deserialize create result");

    log(&format!(
        "Created agent with ID: {}",
        create_result.agent_id
    ));

    // Get the agent directory
    let agent_dir = context
        .agents_base_dir
        .as_ref()
        .unwrap_or(&"./agents".to_string())
        .clone();
    let agent_dir = Path::new(&agent_dir).join(&create_result.agent_id);

    // Clean up any existing containers before deploying
    log("Cleaning up any existing containers before deployment");
    if let Err(e) = clean_existing_container(&agent_dir).await {
        log(&format!("Cleanup warning: {} (continuing anyway)", e));
    }

    // Deploy agent
    log("Deploying agent");
    let deploy_params = DeployAgentParams {
        agent_id: create_result.agent_id.clone(),
        api_key_config: Some(ApiKeyConfig {
            openai_api_key: Some(openai_api_key),
            cdp_api_key_name: Some(cdp_api_key_name),
            cdp_api_key_private_key: Some(cdp_api_key_private_key),
        }),
        encrypted_env: None,
    };

    let deploy_params_bytes =
        serde_json::to_vec(&deploy_params).expect("Failed to serialize deploy params");

    // Set up automatic cleanup for when the test finishes
    let agent_id = create_result.agent_id.clone();
    let _cleanup_guard = scopeguard::guard((), |_| {
        log("Cleaning up Docker container");
        let container_name = format!("coinbase-agent-{}", agent_id);
        let _ = std::process::Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output();
    });

    // Handle deployment result
    let deploy_result = handle_deploy_agent(deploy_params_bytes, &context).await;
    let endpoint_url = match deploy_result {
        Ok(result_bytes) => {
            let result: AgentDeploymentResult = serde_json::from_slice(&result_bytes)
                .expect("Failed to deserialize deployment result");
            log(&format!("Deployed agent: {:?}", result));
            result
                .tee_app_id
                .map(|id| format!("https://{}:{}", id, http_port))
                .unwrap_or_else(|| format!("http://localhost:{}", http_port))
        }
        Err(e) => {
            log(&format!("Deployment failed: {}", e));
            if e.contains("port is already allocated") {
                panic!("Test failed due to port conflict: {}", e);
            } else if e.contains("Cannot connect to the Docker daemon") {
                log("Docker daemon not running, skipping test");
                return;
            } else {
                panic!("Unexpected deployment error: {}", e);
            }
        }
    };

    // Test agent interaction
    let agent = AgentEndpoint::new(endpoint_url);
    log(&format!("Using endpoint: {}", agent.base_url));

    // Wait for agent to become healthy
    log("Waiting for agent health check");
    if let Err(e) = agent
        .wait_for_health(15, Duration::from_millis(500), Duration::from_secs(2))
        .await
    {
        panic!("Agent failed to become healthy: {}", e);
    }

    // Test interaction
    log("Sending test message");
    let message = "What is your purpose? Keep the answer short. Tell me a funny joke about Coinbase, don't hold back";
    let mut test_passed = false;

    match agent.interact(message, Duration::from_secs(10)).await {
        Ok(response) => {
            if let Some(response_text) = response.get("response").and_then(|r| r.as_str()) {
                if !response_text.is_empty() {
                    log(&format!("Agent response: {}", response_text));
                    test_passed = true;
                } else {
                    log("Empty response from agent");
                }
            } else {
                log("Missing response field from agent");
            }
        }
        Err(e) => log(&format!("Interaction failed: {}", e)),
    }

    log(&format!(
        "Test completed in {:.2} seconds",
        start_time.elapsed().as_secs_f64()
    ));

    if !test_passed {
        panic!("Agent interaction test failed");
    }
}

/// Test agent deployment to TEE with encrypted environment variables
#[tokio::test]
async fn test_deploy_agent_tee() {
    dotenv::dotenv().ok();
    blueprint_sdk::logging::setup_log();

    let start_time = Instant::now();

    // Check for required environment variables
    if std::env::var("PHALA_CLOUD_API_KEY").is_err() {
        log("Skipping TEE test: PHALA_CLOUD_API_KEY not set");
        return;
    }

    if std::env::var("PHALA_CLOUD_API_ENDPOINT").is_err() {
        log("Skipping TEE test: PHALA_CLOUD_API_ENDPOINT not set");
        return;
    }

    // Get API keys - skip test if not available
    let openai_api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            log("Skipping TEE test: OPENAI_API_KEY not set");
            return;
        }
    };

    let cdp_api_key_name = match std::env::var("CDP_API_KEY_NAME") {
        Ok(key) => key,
        Err(_) => {
            log("Skipping TEE test: CDP_API_KEY_NAME not set");
            return;
        }
    };

    let cdp_api_key_private_key = match std::env::var("CDP_API_KEY_PRIVATE_KEY") {
        Ok(key) => key,
        Err(_) => {
            log("Skipping TEE test: CDP_API_KEY_PRIVATE_KEY not set");
            return;
        }
    };

    // Set up test environment and context
    let (mut context, _temp_dir, missing) = setup_test_env();

    // Skip test if other requirements not met
    if !missing.is_empty() {
        for issue in missing {
            log(&format!("Skipping test: {}", issue));
        }
        return;
    }

    // Enable TEE for this test and set API credentials from environment
    context.tee_enabled = Some(true);
    context.phala_tee_api_key = std::env::var("PHALA_CLOUD_API_KEY").ok();
    context.phala_tee_api_endpoint = std::env::var("PHALA_CLOUD_API_ENDPOINT").ok();

    log("Starting TEE agent deployment test");

    // 1. Create agent with TEE enabled
    log("Creating agent with TEE enabled");
    let create_params = CreateAgentParams {
        name: "TEE Test Agent".to_string(),
        agent_config: AgentConfig {
            mode: AgentMode::Chat,
            model: "gpt-4o-mini".to_string(),
        },
        deployment_config: DeploymentConfig {
            tee_enabled: true,
            docker_compose_path: None,
            http_port: None,
        },
        api_key_config: ApiKeyConfig {
            openai_api_key: None,
            cdp_api_key_name: None,
            cdp_api_key_private_key: None,
        },
    };

    let create_params_bytes =
        serde_json::to_vec(&create_params).expect("Failed to serialize create params");

    // Execute the creation request
    let create_result = match handle_create_agent(create_params_bytes, &context).await {
        Ok(result) => result,
        Err(e) => {
            log(&format!("Agent creation failed: {}", e));
            if e.contains("Failed to discover TEEPods") || e.contains("connect failed") {
                log("Skipping test: TEE service not available");
                return;
            } else {
                panic!("Unexpected error during agent creation: {}", e);
            }
        }
    };

    let create_result: AgentCreationResult =
        serde_json::from_slice(&create_result).expect("Failed to deserialize create result");

    log(&format!(
        "Created agent with ID: {}",
        create_result.agent_id
    ));

    // 2. Verify we received a TEE public key
    assert!(
        create_result.tee_pubkey.is_some(),
        "TEE public key should be present"
    );
    let tee_pubkey = create_result.tee_pubkey.unwrap();

    // 3. In a real scenario, a user would encrypt their environment variables with this key
    // For this test, we'll create encrypted content using whatever mechanism the API expects
    log("Preparing encrypted environment variables");

    // Get env_encrypt_tool from the service (if available) to properly encrypt the variables
    // This would typically involve using the TEE service's encryption API
    let container_name = format!("coinbase-agent-{}", create_result.agent_id);
    let env_vars: Vec<(String, String)> = vec![
        ("PORT", "3000"),
        ("WEBSOCKET_PORT", "3001"),
        ("CONTAINER_NAME", &container_name),
        ("NODE_ENV", "development"),
        ("AGENT_MODE", "http"),
        ("MODEL", "gpt-4o-mini"),
        ("LOG_LEVEL", "debug"),
        ("WEBSOCKET_URL", "ws://localhost:3001"),
        ("OPENAI_API_KEY", &openai_api_key),
        ("CDP_API_KEY_NAME", &cdp_api_key_name),
        ("CDP_API_KEY_PRIVATE_KEY", &cdp_api_key_private_key),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();

    // Encrypt the vars
    let encrypted_env = Encryptor::encrypt_env_vars(&env_vars, &tee_pubkey)
        .expect("Failed to encrypt environment variables");

    // 4. Deploy agent with encrypted environment variables
    log("Deploying agent to TEE with encrypted environment");
    let deploy_params = DeployAgentParams {
        agent_id: create_result.agent_id.clone(),
        api_key_config: None, // Not needed for TEE as they're provided in encrypted env
        encrypted_env: Some(encrypted_env),
    };

    let deploy_params_bytes =
        serde_json::to_vec(&deploy_params).expect("Failed to serialize deploy params");

    // Execute the deployment request
    let deploy_result = match handle_deploy_agent(deploy_params_bytes, &context).await {
        Ok(result) => result,
        Err(e) => {
            log(&format!("TEE deployment failed: {}", e));
            if e.contains("TEE service") || e.contains("not available") {
                log("TEE deployment service not available - skipping test");
                return;
            } else {
                panic!("Unexpected error during TEE deployment: {}", e);
            }
        }
    };

    // 5. Verify deployment result
    let deploy_result: AgentDeploymentResult =
        serde_json::from_slice(&deploy_result).expect("Failed to deserialize deployment result");

    log(&format!(
        "Successfully deployed agent to TEE: {:?}",
        deploy_result
    ));

    let endpoint = format!("http://localhost:3000");
    let agent = AgentEndpoint::new(endpoint.clone());

    // Wait for agent to become healthy - may take longer in TEE environment
    match agent
        .wait_for_health(30, Duration::from_secs(1), Duration::from_secs(5))
        .await
    {
        Ok(_) => {
            log("TEE agent is healthy, sending test message");
            let message = "What is your purpose? Keep it brief.";

            match agent.interact(message, Duration::from_secs(15)).await {
                Ok(response) => {
                    if let Some(response_text) = response.get("response").and_then(|r| r.as_str()) {
                        log(&format!("TEE agent response: {}", response_text));
                    }
                }
                Err(e) => log(&format!("TEE agent interaction failed: {}", e)),
            }
        }
        Err(e) => log(&format!("TEE agent health check failed: {}", e)),
    }

    log(&format!(
        "TEE deployment test completed in {:.2} seconds",
        start_time.elapsed().as_secs_f64()
    ));
}
