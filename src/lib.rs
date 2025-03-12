use api::services::events::JobCalled;
use blueprint_sdk::config::GadgetConfiguration;
use blueprint_sdk::event_listeners::tangle::events::TangleEventListener;
use blueprint_sdk::event_listeners::tangle::services::{
    services_post_processor, services_pre_processor,
};
use blueprint_sdk::macros::contexts::{ServicesContext, TangleClientContext};
use blueprint_sdk::tangle_subxt::tangle_testnet_runtime::api;

// Public modules
pub mod create_agent;
pub mod deploy_agent;
pub mod docker;
pub mod types;

#[cfg(test)]
mod tests;
// Re-export types and functions
pub use create_agent::handle_create_agent;
pub use deploy_agent::handle_deploy_agent;
pub use types::*;

#[derive(Clone, TangleClientContext, ServicesContext)]
pub struct ServiceContext {
    #[config]
    pub config: GadgetConfiguration,
    #[call_id]
    pub call_id: Option<u64>,
    // Environment variables needed for the service
    pub agents_base_dir: Option<String>,
    pub tee_enabled: Option<bool>,
    pub phala_tee_api_endpoint: Option<String>,
    pub phala_tee_api_key: Option<String>,
}

/// Creates a new Coinbase Agent Kit agent
#[blueprint_sdk::job(
    id = 0,
    params(params),
    result(result),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
        post_processor = services_post_processor,
    ),
)]
pub async fn create_agent(params: Vec<u8>, context: ServiceContext) -> Result<Vec<u8>, String> {
    // Delegate to the implementation in create_agent module
    handle_create_agent(params, &context).await
}

/// Deploys a previously created Coinbase Agent Kit agent
#[blueprint_sdk::job(
    id = 1,
    params(params),
    result(result),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
        post_processor = services_post_processor,
    ),
)]
pub async fn deploy_agent(params: Vec<u8>, context: ServiceContext) -> Result<Vec<u8>, String> {
    // Delegate to the implementation in deploy_agent module
    handle_deploy_agent(params, &context).await
}
