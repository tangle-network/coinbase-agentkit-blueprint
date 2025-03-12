use crate::{
    create_agent::handle_create_agent,
    deploy_agent::handle_deploy_agent,
    docker::{self, AgentEndpoint},
    tests::setup_test_env,
    types::{
        AgentConfig, AgentCreationResult, AgentDeploymentResult, AgentMode, ApiKeyConfig,
        CreateAgentParams, DeployAgentParams, DeploymentConfig,
    },
};
use chrono::Local;
use dotenv::dotenv;
use rand;
use reqwest;
use std::{
    env, thread,
    time::{Duration, Instant},
};

/// Log a message with timestamp
fn log_with_timestamp(msg: &str) {
    let now = Local::now().format("%H:%M:%S%.3f");
    println!("[{}] {}", now, msg);
}

/// Test agent deployment without TEE
#[tokio::test]
async fn test_deploy_agent_local() {
    // Skip test if CI environment is detected
    if env::var("CI").is_ok() {
        println!("Skipping test in CI environment");
        return;
    }

    // Load dotenv from the current directory for the test
    dotenv().ok();

    // First create an agent
    let (context, _temp_dir) = setup_test_env();

    // Create agent parameters
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
            openai_api_key: Some(
                env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test-api-key".to_string()),
            ),
        },
    };

    let create_params_bytes =
        serde_json::to_vec(&create_params).expect("Failed to serialize create params");
    let create_result = handle_create_agent(create_params_bytes, &context).await;
    assert!(
        create_result.is_ok(),
        "Agent creation failed: {:?}",
        create_result.err()
    );

    let create_result_bytes = create_result.unwrap();
    let create_result: AgentCreationResult =
        serde_json::from_slice(&create_result_bytes).expect("Failed to deserialize create result");

    // Now deploy the agent
    let deploy_params = DeployAgentParams {
        agent_id: create_result.agent_id,
        api_key_config: Some(ApiKeyConfig {
            openai_api_key: Some(
                env::var("OPENAI_API_KEY").unwrap_or_else(|_| "test-api-key".to_string()),
            ),
        }),
        encrypted_env_vars: None,
    };

    let deploy_params_bytes =
        serde_json::to_vec(&deploy_params).expect("Failed to serialize deploy params");
    let deploy_result = handle_deploy_agent(deploy_params_bytes, &context).await;

    // Expect a controlled failure about Docker not being available in tests rather than a crash
    match deploy_result {
        Ok(_) => println!("Deployment succeeded unexpectedly - Docker must be available"),
        Err(e) => {
            println!("Expected deployment error: {}", e);
            // Just verify it's an expected type of error (about Docker)
            assert!(
                e.contains("docker-compose") || e.contains("Docker") || e.contains("container"),
                "Error should be related to Docker, got: {}",
                e
            );
        }
    }
}

/// Test agent deployment and interaction with the deployed agent
///
/// Set SKIP_DEPLOY_TESTS=1 to skip this long-running test
#[tokio::test]
async fn test_deploy_agent_interaction() {
    let start_time = Instant::now();

    // Skip if SKIP_DEPLOY_TESTS is set
    if env::var("SKIP_DEPLOY_TESTS").unwrap_or_default() == "1" {
        println!("Skipping deploy agent interaction test due to SKIP_DEPLOY_TESTS=1");
        return;
    }

    // Skip test if CI environment is detected
    if env::var("CI").is_ok() {
        println!("Skipping test in CI environment");
        return;
    }

    log_with_timestamp("Starting deploy agent interaction test");

    // Check if Docker is available by running a simple command
    let docker_available = std::process::Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if !docker_available {
        log_with_timestamp("Skipping test as Docker is not available");
        return;
    }

    // Load dotenv from the current directory for the test
    dotenv().ok();

    // Clean up any existing containers at the start
    log_with_timestamp("Cleaning up any existing containers...");
    let removed = docker::cleanup_containers("coinbase-agent-");
    if removed > 0 {
        log_with_timestamp(&format!("Cleaned up {} existing containers", removed));
    }

    // First create an agent
    log_with_timestamp("Setting up test environment");
    let (context, _temp_dir) = setup_test_env();

    // Create agent parameters with valid OpenAI API key (required for actual interaction)
    let openai_api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() && key != "test-api-key" => key,
        _ => {
            log_with_timestamp("Skipping test as valid OPENAI_API_KEY is not available");
            return;
        }
    };

    // Use random ports to avoid conflicts
    let http_port = 10000 + (rand::random::<u16>() % 1000);
    let websocket_port = http_port + 1;
    log_with_timestamp(&format!(
        "Using random ports for test: HTTP={}, WebSocket={}",
        http_port, websocket_port
    ));

    // Create agent parameters
    log_with_timestamp("Creating agent");
    let create_params = CreateAgentParams {
        name: "Interactive Test Agent".to_string(),
        agent_config: AgentConfig {
            mode: AgentMode::Chat,
            model: "gpt-4o-mini".to_string(), // Use a fast model for testing
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
        },
    };

    let create_params_bytes =
        serde_json::to_vec(&create_params).expect("Failed to serialize create params");
    let create_result = handle_create_agent(create_params_bytes, &context).await;
    assert!(
        create_result.is_ok(),
        "Agent creation failed: {:?}",
        create_result.err()
    );

    let create_result_bytes = create_result.unwrap();
    let create_result: AgentCreationResult =
        serde_json::from_slice(&create_result_bytes).expect("Failed to deserialize create result");

    log_with_timestamp(&format!(
        "Successfully created agent with ID: {}",
        create_result.agent_id
    ));

    // Now deploy the agent
    log_with_timestamp("Deploying agent...");
    let deploy_params = DeployAgentParams {
        agent_id: create_result.agent_id.clone(),
        api_key_config: Some(ApiKeyConfig {
            openai_api_key: Some(openai_api_key),
        }),
        encrypted_env_vars: None,
    };

    let deploy_params_bytes =
        serde_json::to_vec(&deploy_params).expect("Failed to serialize deploy params");
    let deploy_result = handle_deploy_agent(deploy_params_bytes, &context).await;

    // Create a cleanup function to ensure we remove the container at the end
    let agent_id = create_result.agent_id.clone();
    let cleanup = || {
        log_with_timestamp("Cleaning up Docker container...");
        let container_name = format!("coinbase-agent-{}", agent_id);
        let _ = std::process::Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output();
    };

    // Use defer pattern to ensure cleanup happens on all exit paths
    let _cleanup_guard = scopeguard::guard((), |_| {
        cleanup();
    });

    // Check if deployment was successful
    let deployment_result = match deploy_result {
        Ok(result_bytes) => {
            let result: AgentDeploymentResult = serde_json::from_slice(&result_bytes)
                .expect("Failed to deserialize deployment result");
            log_with_timestamp(&format!("Successfully deployed agent: {:?}", result));
            result
        }
        Err(e) => {
            log_with_timestamp(&format!("Agent deployment failed: {}", e));

            // Just checking for the word "Docker" is too broad - we need to check for specific errors
            if e.contains("port is already allocated") {
                panic!("Test failed due to port conflict. Please free up the required ports or restart Docker: {}", e);
            } else if e.contains("Cannot connect to the Docker daemon") {
                log_with_timestamp("Docker daemon not running, skipping interaction test");
                return;
            } else {
                panic!("Unexpected error during deployment: {}", e);
            }
        }
    };

    // Get the endpoint URL
    let endpoint_url = match deployment_result.endpoint {
        Some(url) => url,
        None => format!("http://localhost:{}", http_port), // Use the randomly assigned port
    };

    // Create an agent endpoint helper
    let agent = AgentEndpoint::new(endpoint_url);
    log_with_timestamp(&format!("Using agent endpoint: {}", agent.base_url));

    // Wait for the agent to become healthy
    log_with_timestamp("Waiting for agent to become healthy...");
    if let Err(e) = agent
        .wait_for_health(15, Duration::from_millis(500), Duration::from_secs(2))
        .await
    {
        log_with_timestamp(&format!("Agent health check failed: {}", e));
        panic!("Agent failed to become healthy: {}", e);
    }
    log_with_timestamp("Agent is healthy and ready for interaction");

    // Send a test message to the agent
    log_with_timestamp("Sending test message to agent...");
    let message = "What is your purpose? Keep the answer short.";
    let mut test_passed = false;

    match agent.interact(message, Duration::from_secs(10)).await {
        Ok(response) => {
            log_with_timestamp(&format!("Agent response: {:?}", response));

            if let Some(response_text) = response.get("response").and_then(|r| r.as_str()) {
                if !response_text.is_empty() {
                    log_with_timestamp(&format!(
                        "Agent successfully responded with: {}",
                        response_text
                    ));
                    test_passed = true;
                } else {
                    log_with_timestamp("Agent response text was empty");
                }
            } else {
                log_with_timestamp("Response field missing from agent response");
            }
        }
        Err(e) => {
            log_with_timestamp(&format!("Failed to interact with agent: {}", e));
        }
    }

    // Report total test duration
    let duration = start_time.elapsed();
    log_with_timestamp(&format!(
        "Test completed in {:.2} seconds",
        duration.as_secs_f64()
    ));

    // Make sure the test actually fails if interaction failed
    if !test_passed {
        panic!("Agent interaction test failed");
    }
}
