use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::bot::handlers::TelegramApi;
use crate::config::BotSettings;
use crate::db::{JoinRequestRepo, SessionRepo};
use crate::domain::JoinRequestStatus;
use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
struct ExpiryContext {
    join_request_id: i64,
    join_request_status: JoinRequestStatus,
    join_request_updated_at: DateTime<Utc>,
    telegram_user_chat_id: i64,
    community_title: String,
    community_telegram_chat_id: i64,
    applicant_telegram_user_id: i64,
}

pub async fn run_expiry_loop(
    api: Arc<dyn TelegramApi>,
    pool: PgPool,
    settings: BotSettings,
) {
    tracing::info!("expiry background loop started");
    loop {
        if let Err(err) = process_tick(&*api, &pool, &settings).await {
            tracing::error!(error = %err, "expiry tick failed");
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

pub async fn process_tick(
    api: &dyn TelegramApi,
    pool: &PgPool,
    settings: &BotSettings,
) -> Result<(), AppError> {
    let now = Utc::now();
    process_reminders(api, pool, settings, now).await?;
    process_expired(api, pool, settings, now).await?;
    Ok(())
}

async fn process_reminders(
    api: &dyn TelegramApi,
    pool: &PgPool,
    settings: &BotSettings,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    if settings.reminder_before_expiry_minutes >= settings.application_timeout_minutes {
        return Ok(());
    }

    let reminder_after_minutes =
        settings.application_timeout_minutes - settings.reminder_before_expiry_minutes;
    let reminder_cutoff = now - chrono::Duration::minutes(reminder_after_minutes as i64);

    let contexts = find_needing_reminder(pool, reminder_cutoff).await?;
    if !contexts.is_empty() {
        tracing::info!(count = contexts.len(), "processing reminders");
    }

    for ctx in &contexts {
        if let Err(err) = process_single_reminder(api, pool, ctx).await {
            tracing::error!(
                join_request_id = ctx.join_request_id,
                error = %err,
                "failed to process reminder"
            );
        }
    }

    Ok(())
}

async fn process_single_reminder(
    api: &dyn TelegramApi,
    pool: &PgPool,
    ctx: &ExpiryContext,
) -> Result<(), AppError> {
    let message = format!(
        "Just a reminder — your application to join {} is still pending. \
         Please complete the questionnaire soon, or it will expire.",
        ctx.community_title
    );

    match api.send_message(ctx.telegram_user_chat_id, message).await {
        Ok(()) => {
            JoinRequestRepo::update_reminder_sent_at(pool, ctx.join_request_id).await?;
            tracing::info!(join_request_id = ctx.join_request_id, "reminder sent");
        }
        Err(err) => {
            tracing::warn!(
                join_request_id = ctx.join_request_id,
                error = %err,
                "failed to send reminder message"
            );
        }
    }

    Ok(())
}

async fn process_expired(
    api: &dyn TelegramApi,
    pool: &PgPool,
    settings: &BotSettings,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let expiry_cutoff = now - chrono::Duration::minutes(settings.application_timeout_minutes as i64);
    let contexts = find_expired_contexts(pool, expiry_cutoff).await?;

    if !contexts.is_empty() {
        tracing::info!(count = contexts.len(), "processing expirations");
    }

    for ctx in &contexts {
        if let Err(err) = process_single_expiry(api, pool, ctx).await {
            tracing::error!(
                join_request_id = ctx.join_request_id,
                error = %err,
                "failed to process expiry"
            );
        }
    }

    Ok(())
}

async fn process_single_expiry(
    api: &dyn TelegramApi,
    pool: &PgPool,
    ctx: &ExpiryContext,
) -> Result<(), AppError> {
    match JoinRequestRepo::update_status(
        pool,
        ctx.join_request_id,
        ctx.join_request_status,
        JoinRequestStatus::Expired,
        ctx.join_request_updated_at,
    )
    .await
    {
        Ok(_) => {}
        Err(AppError::AlreadyProcessed { .. }) => {
            tracing::info!(
                join_request_id = ctx.join_request_id,
                "join request already processed, skipping expiry"
            );
            return Ok(());
        }
        Err(err) => return Err(err),
    }

    if let Ok(Some(session)) =
        SessionRepo::find_active_by_join_request_id(pool, ctx.join_request_id).await
    {
        if let Err(err) = SessionRepo::expire(pool, session.id).await {
            tracing::warn!(
                join_request_id = ctx.join_request_id,
                session_id = session.id,
                error = %err,
                "failed to expire session"
            );
        }
    }

    let message = format!(
        "Your application to join {} timed out. You can request to join again if you'd like.",
        ctx.community_title
    );
    if let Err(err) = api.send_message(ctx.telegram_user_chat_id, message).await {
        tracing::warn!(
            join_request_id = ctx.join_request_id,
            error = %err,
            "failed to send expiry message"
        );
    }

    if let Err(err) = api
        .decline_chat_join_request(
            ctx.community_telegram_chat_id,
            ctx.applicant_telegram_user_id,
        )
        .await
    {
        tracing::warn!(
            join_request_id = ctx.join_request_id,
            error = %err,
            "failed to decline expired join request on telegram"
        );
    }

    tracing::info!(
        join_request_id = ctx.join_request_id,
        status = %ctx.join_request_status,
        "join request expired"
    );

    Ok(())
}

async fn find_expired_contexts(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
) -> Result<Vec<ExpiryContext>, AppError> {
    let rows = sqlx::query_as::<_, ExpiryContext>(
        r#"SELECT jr.id AS join_request_id,
                  jr.status AS join_request_status,
                  jr.updated_at AS join_request_updated_at,
                  jr.telegram_user_chat_id,
                  c.title AS community_title,
                  c.telegram_chat_id AS community_telegram_chat_id,
                  a.telegram_user_id AS applicant_telegram_user_id
           FROM join_requests jr
           INNER JOIN communities c ON c.id = jr.community_id
           INNER JOIN applicants a ON a.id = jr.applicant_id
           WHERE jr.status IN ('pending_contact', 'questionnaire_in_progress')
             AND jr.created_at < $1"#,
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

async fn find_needing_reminder(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
) -> Result<Vec<ExpiryContext>, AppError> {
    let rows = sqlx::query_as::<_, ExpiryContext>(
        r#"SELECT jr.id AS join_request_id,
                  jr.status AS join_request_status,
                  jr.updated_at AS join_request_updated_at,
                  jr.telegram_user_chat_id,
                  c.title AS community_title,
                  c.telegram_chat_id AS community_telegram_chat_id,
                  a.telegram_user_id AS applicant_telegram_user_id
           FROM join_requests jr
           INNER JOIN communities c ON c.id = jr.community_id
           INNER JOIN applicants a ON a.id = jr.applicant_id
           WHERE jr.status = 'questionnaire_in_progress'
             AND jr.created_at < $1
             AND jr.reminder_sent_at IS NULL"#,
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
