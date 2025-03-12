use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// A struct representing a deployed agent endpoint
#[derive(Debug, Clone)]
pub struct AgentEndpoint {
    /// Base URL of the agent (e.g., http://localhost:3000)
    pub base_url: String,
    /// HTTP client for making requests
    http_client: reqwest::Client,
}

impl AgentEndpoint {
    /// Creates a new AgentEndpoint
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the agent (e.g., "http://localhost:3000")
    ///
    /// # Returns
    ///
    /// A new AgentEndpoint instance
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Creates an AgentEndpoint from a port number (localhost)
    ///
    /// # Arguments
    ///
    /// * `port` - The HTTP port the agent is listening on
    ///
    /// # Returns
    ///
    /// A new AgentEndpoint instance
    pub fn from_port(port: u16) -> Self {
        Self::new(format!("http://localhost:{}", port))
    }

    /// Checks if the agent's health endpoint is responding with detailed diagnostics
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for a response
    ///
    /// # Returns
    ///
    /// A Result containing the health status or an error
    pub async fn check_health(&self, timeout: Duration) -> Result<Value, String> {
        let health_url = format!("{}/health", self.base_url);

        // Log the actual request we're making
        blueprint_sdk::logging::info!("Sending health check request to: {}", health_url);

        // Build the request with timeout
        let request = self.http_client.get(&health_url).timeout(timeout);

        // Try to send the request and handle different error cases
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                blueprint_sdk::logging::info!("Health check response status: {}", status);

                if status.is_success() {
                    // Try to parse the response as JSON
                    match response.json::<Value>().await {
                        Ok(json) => {
                            blueprint_sdk::logging::info!(
                                "Health check successful with response: {:?}",
                                json
                            );
                            Ok(json)
                        }
                        Err(e) => {
                            blueprint_sdk::logging::warn!(
                                "Health check returned non-JSON response: {}",
                                e
                            );
                            Err(format!("Failed to parse health response: {}", e))
                        }
                    }
                } else {
                    // Handle non-200 responses
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Could not read response body".to_string());
                    blueprint_sdk::logging::warn!(
                        "Health check failed with status {} and body: {}",
                        status,
                        error_text
                    );
                    Err(format!(
                        "Health check returned error status: {} with body: {}",
                        status, error_text
                    ))
                }
            }
            Err(e) => {
                // Add more context based on the type of error
                if e.is_timeout() {
                    blueprint_sdk::logging::warn!("Health check timed out after {:?}", timeout);
                    Err(format!("Health check timed out after {:?}: {}", timeout, e))
                } else if e.is_connect() {
                    blueprint_sdk::logging::warn!("Connection error during health check: {}", e);
                    Err(format!("Connection error during health check: {}", e))
                } else {
                    blueprint_sdk::logging::warn!("Health check request failed: {}", e);
                    Err(format!("Health check request failed: {}", e))
                }
            }
        }
    }

    /// Waits for the agent to become healthy with detailed diagnostics
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - Maximum number of health check attempts
    /// * `initial_delay` - Time to wait before the first attempt
    /// * `timeout` - Maximum time to wait for each health check response
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error message
    pub async fn wait_for_health(
        &self,
        max_attempts: u32,
        initial_delay: Duration,
        timeout: Duration,
    ) -> Result<(), String> {
        // Wait before first attempt
        tokio::time::sleep(initial_delay).await;

        // Track start time for overall statistics
        let start_time = Instant::now();

        for attempt in 1..=max_attempts {
            blueprint_sdk::logging::info!(
                "Health check attempt {} of {} for {}",
                attempt,
                max_attempts,
                self.base_url
            );

            match self.check_health(timeout).await {
                Ok(_) => {
                    let duration = start_time.elapsed();
                    blueprint_sdk::logging::info!(
                        "Agent became healthy after {} attempts ({}ms)",
                        attempt,
                        duration.as_millis()
                    );
                    return Ok(());
                }
                Err(e) => {
                    blueprint_sdk::logging::warn!("Health check attempt {} failed: {}", attempt, e);

                    // If this isn't the last attempt, wait before trying again
                    if attempt < max_attempts {
                        // Increase delay with each failure using exponential backoff
                        let delay = initial_delay.mul_f32(1.5_f32.powi(attempt as i32 - 1));
                        blueprint_sdk::logging::info!("Waiting {:?} before next attempt", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        let total_duration = start_time.elapsed();
        blueprint_sdk::logging::error!(
            "Agent failed to become healthy after {} attempts ({}ms total time)",
            max_attempts,
            total_duration.as_millis()
        );

        Err(format!(
            "Agent failed to become healthy after {} attempts",
            max_attempts
        ))
    }

    /// Sends a message to the agent and gets a response
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send to the agent
    /// * `timeout` - Maximum time to wait for a response
    ///
    /// # Returns
    ///
    /// A Result containing the agent's response or an error
    pub async fn interact(&self, message: &str, timeout: Duration) -> Result<Value, String> {
        let interact_url = format!("{}/interact", self.base_url);
        self.http_client
            .post(&interact_url)
            .json(&json!({ "message": message }))
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| format!("Interaction request failed: {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse interaction response: {}", e))
    }
}

/// Enum representing the type of deployment (local Docker or TEE)
#[derive(Debug, Clone, PartialEq)]
pub enum DeploymentType {
    /// Local Docker deployment
    Local,
    /// TEE deployment
    Tee,
}
