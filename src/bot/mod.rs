pub mod handlers;

use std::sync::Arc;

use sqlx::PgPool;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::dispatching::UpdateHandler;
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::*;
use teloxide::types::{AllowedUpdate, Message};

use crate::config::Config;
use crate::error::AppError;

pub fn schema() -> UpdateHandler<AppError> {
    let private_message_handler = Update::filter_message()
        .filter(handlers::is_private_chat)
        .branch(
            teloxide::dptree::filter(|msg: Message| {
                msg.text()
                    .map(|text| text.trim_start().starts_with("/start"))
                    .unwrap_or(false)
            })
            .endpoint(handlers::start::handle_start),
        )
        .endpoint(handlers::handle_private_message);

    teloxide::dptree::entry()
        .branch(
            Update::filter_chat_join_request()
                .endpoint(handlers::join_request::handle_join_request),
        )
        .branch(private_message_handler)
        .branch(Update::filter_callback_query().endpoint(handlers::handle_callback_query))
}

pub async fn run_dispatcher(bot: Bot, pool: PgPool, config: Config) -> Result<(), AppError> {
    bot.delete_webhook()
        .await
        .map_err(|err| AppError::Telegram(err.to_string()))?;

    let listener = teloxide::update_listeners::Polling::builder(bot.clone())
        .allowed_updates(vec![
            AllowedUpdate::Message,
            AllowedUpdate::CallbackQuery,
            AllowedUpdate::ChatJoinRequest,
        ])
        .build();

    let mut dispatcher = Dispatcher::builder(bot, schema())
        .dependencies(teloxide::dptree::deps![pool, Arc::new(config)])
        .distribution_function(|upd: &Update| upd.from().map(|user| user.id.0))
        .default_handler(|upd| async move {
            tracing::warn!(update = ?upd, "unhandled update");
        })
        .build();

    dispatcher
        .dispatch_with_listener(
            listener,
            LoggingErrorHandler::with_custom_text("dispatcher error"),
        )
        .await;

    Ok(())
}
