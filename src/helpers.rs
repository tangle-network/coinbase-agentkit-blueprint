use blueprint_sdk::logging;

use crate::{agent_endpoint::AgentEndpoint, docker};

/// Inspect container environment variables to debug CDP API credentials
pub async fn inspect_container_env(container_name: &str) -> Result<String, String> {
    logging::info!("Inspecting container environment for CDP credentials...");

    // First check if container is running
    let status_cmd = tokio::process::Command::new("docker")
        .args(&["inspect", "--format", "{{.State.Status}}", container_name])
        .output()
        .await
        .map_err(|e| format!("Failed to check container status: {}", e))?;

    let status = String::from_utf8_lossy(&status_cmd.stdout)
        .trim()
        .to_string();
    if status != "running" {
        return Err(format!(
            "Container is not running, current status: {}",
            status
        ));
    }

    logging::info!("Container status is '{}', checking environment...", status);

    // Get environment variables from container
    let env_cmd = tokio::process::Command::new("docker")
        .args(&["exec", container_name, "env"])
        .output()
        .await
        .map_err(|e| format!("Failed to get container environment: {}", e))?;

    let env_output = String::from_utf8_lossy(&env_cmd.stdout).to_string();

    // Check for CDP variables specifically
    let mut result = String::new();
    result.push_str(&format!(
        "Environment variables in container '{}':\n",
        container_name
    ));

    // Find CDP credential variables but redact actual values
    for line in env_output.lines() {
        if line.starts_with("CDP_API_KEY_NAME=") {
            result.push_str("CDP_API_KEY_NAME=***REDACTED***\n");
            logging::info!("Found CDP_API_KEY_NAME in container environment");
        } else if line.starts_with("CDP_API_KEY_PRIVATE_KEY=") {
            result.push_str("CDP_API_KEY_PRIVATE_KEY=***REDACTED***\n");
            logging::info!("Found CDP_API_KEY_PRIVATE_KEY in container environment");
        }
    }

    // If variables not found, make it explicit
    if !env_output.contains("CDP_API_KEY_NAME=") {
        result.push_str("CDP_API_KEY_NAME not found in container environment!\n");
        logging::error!("CDP_API_KEY_NAME not found in container environment!");
    }

    if !env_output.contains("CDP_API_KEY_PRIVATE_KEY=") {
        result.push_str("CDP_API_KEY_PRIVATE_KEY not found in container environment!\n");
        logging::error!("CDP_API_KEY_PRIVATE_KEY not found in container environment!");
    }

    Ok(result)
}

/// Collect comprehensive diagnostics about the container status
pub async fn collect_container_diagnostics(
    container_name: &str,
    port: u16,
) -> Result<String, String> {
    let mut diagnostics = String::new();

    // Wait a moment for container to initialize
    logging::info!("Waiting for container to initialize...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check container state
    let inspect_output = tokio::process::Command::new("docker")
        .args(&["inspect", "--format", "{{.State.Status}}", container_name])
        .output()
        .await
        .map_err(|e| format!("Failed to inspect container: {}", e))?;

    let state = String::from_utf8_lossy(&inspect_output.stdout)
        .trim()
        .to_string();
    diagnostics.push_str(&format!("Container state: {}\n", state));

    // Check if port is actually bound
    let port_check = tokio::process::Command::new("docker")
        .args(&["exec", container_name, "netstat", "-tuln"])
        .output()
        .await;

    if let Ok(output) = port_check {
        let ports = String::from_utf8_lossy(&output.stdout);
        diagnostics.push_str(&format!("Container ports:\n{}\n", ports));

        if !ports.contains(&format!(":{}", port)) {
            diagnostics.push_str(&format!(
                "WARNING: Expected port {} not found in netstat output\n",
                port
            ));
        }
    }

    // Check host port binding
    let netstat_output = tokio::process::Command::new("lsof")
        .args(&["-i", &format!(":{}", port)])
        .output()
        .await;

    if let Ok(output) = netstat_output {
        let host_ports = String::from_utf8_lossy(&output.stdout);
        diagnostics.push_str(&format!("Host port {} status:\n{}\n", port, host_ports));

        if host_ports.is_empty() {
            diagnostics.push_str(&format!(
                "WARNING: No process is listening on port {} on the host\n",
                port
            ));
        }
    }

    // Check if container is restarting
    let restart_output = tokio::process::Command::new("docker")
        .args(&["inspect", "--format", "{{.RestartCount}}", container_name])
        .output()
        .await;

    if let Ok(output) = restart_output {
        let restart_count = String::from_utf8_lossy(&output.stdout).trim().to_string();
        diagnostics.push_str(&format!("Container restart count: {}\n", restart_count));

        if restart_count != "0" {
            diagnostics.push_str("WARNING: Container has restarted, indicating potential issues\n");
        }
    }

    Ok(diagnostics)
}

/// Helper function to check if an agent is healthy
pub async fn check_agent_health(endpoint: &str) -> Result<(), String> {
    logging::info!("Starting health check for endpoint: {}", endpoint);
    let agent = AgentEndpoint::new(endpoint);

    // Improved health check parameters
    let max_attempts = 15;
    let initial_delay = std::time::Duration::from_millis(1000); // Increased initial delay
    let max_delay = std::time::Duration::from_secs(5);
    let timeout = std::time::Duration::from_secs(5); // Increased timeout

    logging::info!(
        "Health check parameters: {} attempts, {}ms initial delay, {}s max delay, {}s timeout",
        max_attempts,
        initial_delay.as_millis(),
        max_delay.as_secs(),
        timeout.as_secs()
    );

    // First check if the endpoint is reachable at TCP level
    logging::info!("Checking TCP connectivity to {}", endpoint);
    let url = url::Url::parse(endpoint).map_err(|e| format!("Invalid URL: {}", e))?;
    let host = url.host_str().ok_or("No host in URL")?;
    let port = url.port().unwrap_or(80);
    let addr = format!("{}:{}", host, port);

    // Try to establish TCP connection first
    let tcp_result = tokio::net::TcpStream::connect(&addr).await;
    match tcp_result {
        Ok(_) => logging::info!("TCP connection successful to {}", addr),
        Err(e) => {
            logging::warn!("TCP connection failed to {}: {}", addr, e);
            // Continue with HTTP checks anyway, but log the TCP failure
        }
    }

    // Check container logs for specific error patterns
    let container_name = format!("coinbase-agent-*");
    let logs_check = tokio::process::Command::new("docker")
        .args(&[
            "logs",
            "--tail",
            "20",
            "--filter",
            &format!("name={}", container_name),
        ])
        .output()
        .await;

    if let Ok(output) = logs_check {
        let logs = String::from_utf8_lossy(&output.stdout);
        if logs.contains("Failed to initialize wallet: APIError") {
            logging::error!("DETECTED ERROR: Wallet initialization is failing - CDP API credentials may be invalid or missing");
            logging::error!(
                "Please check that CDP_API_KEY_NAME and CDP_API_KEY_PRIVATE_KEY are correctly set"
            );
            logging::info!("Trying to retrieve CDP variables from container environment...");

            let env_check = tokio::process::Command::new("docker")
                .args(&["exec", "coinbase-agent-*", "env", "|", "grep", "CDP"])
                .output()
                .await;

            if let Ok(env_output) = env_check {
                let env_vars = String::from_utf8_lossy(&env_output.stdout);
                if env_vars.is_empty() {
                    logging::error!("CDP variables not found in container environment!");
                } else {
                    // Redact the actual values for security
                    logging::info!("CDP variables found (values redacted)");
                }
            }
        }
    }

    // Try each health check attempt with detailed logging
    for attempt in 1..=max_attempts {
        logging::info!(
            "Starting health check attempt {} of {}",
            attempt,
            max_attempts
        );

        // Add a delay between attempts that increases with each failure
        if attempt > 1 {
            let delay = std::cmp::min(
                initial_delay.mul_f32(1.5_f32.powi(attempt as i32 - 1)),
                max_delay,
            );
            logging::info!("Waiting {}ms before next attempt", delay.as_millis());
            tokio::time::sleep(delay).await;
        }

        // Try a basic HTTP GET first to see if server responds at all
        if attempt % 3 == 1 {
            // Every 3rd attempt, try a basic GET
            match reqwest::Client::new()
                .get(endpoint)
                .timeout(timeout)
                .send()
                .await
            {
                Ok(resp) => {
                    logging::info!("Basic HTTP GET successful: status {}", resp.status());
                    if resp.status().is_success() || resp.status().as_u16() == 404 {
                        // Even a 404 means the server is up, just not that specific path
                        logging::info!("Server is responding to HTTP requests");
                    }
                }
                Err(e) => logging::warn!("Basic HTTP GET failed: {}", e),
            }
        }

        // Perform the actual health check
        match agent.check_health(timeout).await {
            Ok(_) => {
                logging::info!("Health check successful on attempt {}", attempt);
                return Ok(());
            }
            Err(e) => {
                // Provide more context about the error
                logging::warn!("Health check attempt {} failed: {}", attempt, e);

                // Try to determine if this is a network error or application error
                if e.to_string().contains("connection refused") {
                    logging::warn!("Container may not be listening on port (connection refused)");
                } else if e.to_string().contains("connection reset")
                    || e.to_string().contains("connection closed")
                    || e.to_string().contains("broken pipe")
                {
                    logging::warn!(
                        "Connection was reset - container may be starting up or crashing"
                    );
                } else if e.to_string().contains("timed out") {
                    logging::warn!(
                        "Connection timed out - container may be overloaded or unresponsive"
                    );
                }
            }
        }
    }

    // If we reach here, health check failed after all attempts
    let error_msg = format!(
        "Agent failed to become healthy after {} attempts",
        max_attempts
    );
    logging::error!("{}", error_msg);

    Err(error_msg)
}
