use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

// Agent configuration types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentMode {
    Autonomous,
    Chat,
}

// Implement Display for AgentMode
impl fmt::Display for AgentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentMode::Autonomous => write!(f, "Autonomous"),
            AgentMode::Chat => write!(f, "Chat"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentConfig {
    pub mode: AgentMode,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeeConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub api_endpoint: Option<String>,
    pub teepod_id: Option<u64>,
    pub app_id: Option<String>,
    pub pubkey: Option<String>,
    pub encrypted_env: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub tee_enabled: bool,
    pub docker_compose_path: Option<PathBuf>,
    pub public_key: Option<String>,
    pub http_port: Option<u16>,
    pub tee_config: Option<TeeConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    pub openai_api_key: Option<String>,
}

// Job parameters and results
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAgentParams {
    pub name: String,
    pub agent_config: AgentConfig,
    pub deployment_config: DeploymentConfig,
    pub api_key_config: ApiKeyConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeployAgentParams {
    pub agent_id: String,
    pub api_key_config: Option<ApiKeyConfig>,
    pub encrypted_env_vars: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentCreationResult {
    pub agent_id: String,
    pub files_created: Vec<String>,
    pub tee_public_key: Option<String>,
    pub tee_pubkey: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentDeploymentResult {
    pub agent_id: String,
    pub deployment_id: String,
    pub endpoint: Option<String>,
    pub tee_pubkey: Option<String>,
    pub tee_app_id: Option<String>,
}

// Query types (for non-state-changing operations)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStatusQuery {
    pub agent_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub status: String,
    pub uptime: Option<u64>,
    pub mode: AgentMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentInteractionQuery {
    pub agent_id: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentInteractionResponse {
    pub response: String,
}
