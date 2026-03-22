use std::sync::Arc;

use verifier_bot::bot::handlers::{TelegramApi, TeloxideApi};
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

    let expiry_api: Arc<dyn TelegramApi> = Arc::new(TeloxideApi::new(bot.clone()));
    let expiry_pool = pool.clone();
    let expiry_settings = config.bot_settings.clone();
    tokio::spawn(async move {
        verifier_bot::services::expiry::run_expiry_loop(expiry_api, expiry_pool, expiry_settings)
            .await;
    });

    let result = if config.use_webhooks {
        tracing::info!("starting in webhook mode");
        verifier_bot::bot::run_webhook(bot, pool, config).await
    } else {
        tracing::info!("starting in polling mode");
        verifier_bot::bot::run_polling(bot, pool, config).await
    };

    result.map_err(|err: AppError| anyhow::anyhow!(err.to_string()))?;

    tracing::info!("verifier-bot stopped");
    Ok(())
}
