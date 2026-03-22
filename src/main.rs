use verifier_bot::config::Config;
use verifier_bot::db::{create_pool, run_migrations};
use verifier_bot::error::AppError;

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

    let pool = create_pool(&config.database_url).await?;
    run_migrations(&pool).await?;
    verifier_bot::db::sync::sync_config_to_db(&pool, &config.communities).await?;

    let bot = teloxide::Bot::new(config.bot_token.clone());

    verifier_bot::bot::run_dispatcher(bot, pool, config)
        .await
        .map_err(|err: AppError| anyhow::anyhow!(err.to_string()))?;

    Ok(())
}
