pub mod handlers;
pub mod shutdown;
pub mod webhook;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::PgPool;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::dispatching::UpdateHandler;
use teloxide::error_handlers::{ErrorHandler, LoggingErrorHandler};
use teloxide::prelude::*;
use teloxide::types::{AllowedUpdate, Message};
use teloxide::update_listeners::webhooks;
use teloxide::update_listeners::Polling;

use crate::config::Config;
use crate::error::AppError;

use self::shutdown::{shutdown_signal, ShutdownSignal};
use self::webhook::health_router;

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

fn allowed_updates() -> Vec<AllowedUpdate> {
    vec![
        AllowedUpdate::Message,
        AllowedUpdate::CallbackQuery,
        AllowedUpdate::ChatJoinRequest,
    ]
}

pub struct CustomErrorHandler;

impl ErrorHandler<AppError> for CustomErrorHandler {
    fn handle_error(
        self: Arc<Self>,
        error: AppError,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            let is_join_request_error = matches!(
                &error,
                AppError::InvalidStateTransition { .. } | AppError::AlreadyProcessed { .. }
            ) || error.to_string().to_lowercase().contains("join");

            if is_join_request_error {
                tracing::error!(
                    error = %error,
                    category = "chat_join_request",
                    "critical: join request processing failed"
                );
            } else {
                tracing::error!(error = %error, "handler error");
            }
        })
    }
}

fn build_dispatcher(bot: Bot, pool: PgPool, config: Config) -> Dispatcher<Bot, AppError, u64> {
    Dispatcher::builder(bot, schema())
        .dependencies(teloxide::dptree::deps![pool, Arc::new(config)])
        .distribution_function(|upd: &Update| upd.from().map(|user| user.id.0))
        .error_handler(Arc::new(CustomErrorHandler))
        .default_handler(|upd| async move {
            tracing::warn!(update = ?upd, "unhandled update");
        })
        .build()
}

pub async fn run_polling(bot: Bot, pool: PgPool, config: Config) -> Result<(), AppError> {
    bot.delete_webhook()
        .await
        .map_err(|err| AppError::Telegram(err.to_string()))?;

    let server_port = config.server_port;
    let shutdown = ShutdownSignal::new();
    let mut shutdown_for_health = shutdown.clone();

    tokio::spawn(async move {
        let app = health_router("polling");
        let Ok(tcp) =
            tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], server_port))).await
        else {
            tracing::error!(port = server_port, "failed to bind health server");
            return;
        };
        tracing::info!(port = server_port, mode = "polling", "health server started");
        let _ = axum::serve(tcp, app)
            .with_graceful_shutdown(async move { shutdown_for_health.wait().await })
            .await;
    });

    let listener = Polling::builder(bot.clone())
        .allowed_updates(allowed_updates())
        .build();

    let mut dispatcher = build_dispatcher(bot, pool, config);
    let shutdown_token = dispatcher.shutdown_token();

    tokio::select! {
        _ = shutdown_signal() => {
            let _ = shutdown_token.shutdown();
        }
        _ = dispatcher.dispatch_with_listener(
            listener,
            LoggingErrorHandler::with_custom_text("listener error"),
        ) => {
            tracing::info!("dispatcher exited");
        }
    }

    shutdown.shutdown();
    Ok(())
}

pub async fn run_webhook(bot: Bot, pool: PgPool, config: Config) -> Result<(), AppError> {
    let webhook_url = config
        .public_webhook_url
        .as_ref()
        .ok_or_else(|| {
            AppError::Internal("PUBLIC_WEBHOOK_URL required for webhook mode".into())
        })?
        .clone();

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    let url: url::Url = webhook_url
        .parse()
        .map_err(|e: url::ParseError| AppError::Internal(format!("invalid webhook URL: {e}")))?;

    let options = webhooks::Options::new(addr, url);
    let (listener, stop_signal, webhook_router) = webhooks::axum_to_router(bot.clone(), options)
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?;

    let app = webhook_router.merge(health_router("webhook"));

    let tcp = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::Internal(format!("failed to bind webhook server: {e}")))?;

    tracing::info!(%addr, url = %webhook_url, "webhook server started");

    let shutdown = ShutdownSignal::new();
    let mut shutdown_for_server = shutdown.clone();

    let server_handle = tokio::spawn(async move {
        axum::serve(tcp, app)
            .with_graceful_shutdown(async move { shutdown_for_server.wait().await })
            .await
    });

    let mut dispatcher = build_dispatcher(bot, pool, config);
    let shutdown_token = dispatcher.shutdown_token();

    tokio::select! {
        _ = shutdown_signal() => {
            let _ = shutdown_token.shutdown();
        }
        _ = dispatcher.dispatch_with_listener(
            listener,
            LoggingErrorHandler::with_custom_text("listener error"),
        ) => {
            tracing::info!("webhook dispatcher exited");
        }
    }

    shutdown.shutdown();
    let _ = server_handle.await;
    stop_signal.await;

    Ok(())
}
