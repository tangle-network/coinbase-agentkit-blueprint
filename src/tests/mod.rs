// Integration tests for agent lifecycle

mod agent_lifecycle_test;

use crate::types::{AgentConfig, AgentMode};

#[test]
fn test_agent_config() {
    let config = AgentConfig {
        mode: AgentMode::Autonomous,
        model: "gpt-4o-mini".to_string(),
    };

    assert!(matches!(config.mode, AgentMode::Autonomous));
}
