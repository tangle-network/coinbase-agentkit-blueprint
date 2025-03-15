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
pub struct DeploymentConfig {
    pub tee_enabled: bool,
    pub docker_compose_path: Option<PathBuf>,
    pub http_port: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    pub openai_api_key: Option<String>,
    pub cdp_api_key_name: Option<String>,
    pub cdp_api_key_private_key: Option<String>,
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
    pub encrypted_env: Option<String>,
    pub tee_pubkey: Option<String>,
    pub tee_app_id: Option<String>,
    pub tee_salt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentCreationResult {
    pub agent_id: String,
    pub files_created: Vec<String>,
    pub tee_pubkey: Option<String>,
    pub tee_app_id: Option<String>,
    pub tee_salt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentDeploymentResult {
    pub agent_id: String,
    pub tee_pubkey: Option<String>,
    pub tee_app_id: Option<String>,
}
