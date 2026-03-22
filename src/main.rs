use verifier_bot::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::load()?;

    verifier_bot::logging::init(&config.rust_log);

    tracing::info!(
        communities = config.communities.len(),
        webhooks = config.use_webhooks,
        "verifier-bot starting"
    );

    Ok(())
}
