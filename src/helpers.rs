use blueprint_sdk::logging;
use std::process::Command;
use tokio::process::Command as TokioCommand;

use crate::{agent_endpoint::AgentEndpoint, docker};

/// Check if a Docker container is running
///
/// # Returns
///
/// - `Ok(true)` if the container is running
/// - `Ok(false)` if the container exists but is not running
/// - `Err(String)` if there was an error checking the container status
pub fn check_container_status(container_name: &str) -> Result<bool, String> {
    let output = Command::new("docker")
        .args(&[
            "ps",
            "-a",
            "--filter",
            &format!("name={}", container_name),
            "--format",
            "{{.Status}}",
        ])
        .output()
        .map_err(|e| format!("Failed to execute docker ps command: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Docker ps command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if status.is_empty() {
        return Ok(false); // Container does not exist
    }

    // Container status usually starts with "Up" if it's running
    Ok(status.starts_with("Up"))
}

/// Get logs from a Docker container and check for specific error patterns
///
/// # Returns
///
/// - The container logs as a String
/// - An error message if something went wrong
pub fn get_container_logs(container_name: &str) -> Result<String, String> {
    let output = Command::new("docker")
        .args(&["logs", container_name])
        .output()
        .map_err(|e| format!("Failed to get container logs: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to get container logs: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let logs = String::from_utf8_lossy(&output.stdout).to_string();

    // Check for common error patterns in the logs
    if logs.contains("Failed to initialize wallet") {
        logging::error!("Detected wallet initialization failure in logs");
    } else if logs.contains("Error: connect ECONNREFUSED") {
        logging::error!("Detected connection refused error in logs");
    } else if logs.contains("429 Too Many Requests") {
        logging::error!("Detected rate limit error in logs");
    }

    Ok(logs)
}

/// Simplified function to check if an agent is healthy
pub async fn check_agent_health(endpoint: &str) -> Result<(), String> {
    logging::info!("Starting health check for endpoint: {}", endpoint);
    let agent = AgentEndpoint::new(endpoint);

    // Health check parameters
    let max_attempts = 10;
    let delay_between_attempts = std::time::Duration::from_secs(3);
    let timeout = std::time::Duration::from_secs(5);

    // First, give the container some time to start up
    logging::info!("Waiting for container to initialize (5s)...");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Try each health check attempt
    for attempt in 1..=max_attempts {
        logging::info!("Health check attempt {}/{}", attempt, max_attempts);

        match agent.check_health(timeout).await {
            Ok(_) => {
                logging::info!("Agent health check passed on attempt {}", attempt);
                return Ok(());
            }
            Err(e) => {
                if attempt == max_attempts {
                    let error_msg = format!(
                        "Agent health check failed after {} attempts: {}",
                        max_attempts, e
                    );
                    logging::error!("{}", error_msg);
                    return Err(error_msg);
                }

                logging::warn!("Health check attempt {} failed: {}", attempt, e);
                logging::info!(
                    "Waiting {}s before next attempt...",
                    delay_between_attempts.as_secs()
                );
                tokio::time::sleep(delay_between_attempts).await;
            }
        }
    }

    // This should never be reached due to the return in the loop
    Err(format!(
        "Agent failed to become healthy after {} attempts",
        max_attempts
    ))
}
