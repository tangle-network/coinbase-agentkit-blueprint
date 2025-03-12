use crate::{
    agent_endpoint::AgentEndpoint,
    create_agent::handle_create_agent,
    deploy_agent::handle_deploy_agent,
    tests::setup_test_env,
    types::{
        AgentConfig, AgentCreationResult, AgentDeploymentResult, AgentMode, ApiKeyConfig,
        CreateAgentParams, DeployAgentParams, DeploymentConfig,
    },
};
use chrono::Local;
use rand;
use std::{
    env,
    time::{Duration, Instant},
};

/// Log a message with timestamp for test output
fn log(msg: &str) {
    println!("[{}] {}", Local::now().format("%H:%M:%S%.3f"), msg);
}

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
            public_key: None,
            http_port: Some(http_port),
            tee_config: None,
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

    // Set up automatic cleanup
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
                .endpoint
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
    let message = "What is your purpose? Keep the answer short.";
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
