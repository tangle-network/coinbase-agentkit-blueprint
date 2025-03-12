use blueprint_sdk::logging;
use blueprint_sdk::runners::core::runner::BlueprintRunner;
use blueprint_sdk::runners::tangle::tangle::TangleConfig;
use coinbase_agent_kit_blueprint as blueprint;
use std::env;
use std::path::PathBuf;

#[blueprint_sdk::main(env)]
async fn main() {
    // Create service context
    let context = blueprint::ServiceContext {
        config: env.clone(),
        call_id: None,
        agents_base_dir: None,
        tee_enabled: None,
        phala_tee_api_endpoint: None,
        phala_tee_api_key: None,
    };

    // Create event handlers from jobs
    let create_agent_job = blueprint::CreateAgentEventHandler::new(&env, context.clone()).await?;
    let deploy_agent_job = blueprint::DeployAgentEventHandler::new(&env, context.clone()).await?;

    logging::info!("Starting event watchers for jobs...");
    let tangle_config = TangleConfig::default();
    BlueprintRunner::new(tangle_config, env)
        .job(create_agent_job)
        .job(deploy_agent_job)
        .run()
        .await?;

    logging::info!("Exiting...");
    Ok(())
}
