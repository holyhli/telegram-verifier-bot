use chrono::{DateTime, Utc};
use sqlx::PgPool;
use teloxide::prelude::Bot;
use teloxide::types::Message;

use crate::db::{JoinRequestRepo, SessionRepo};
use crate::domain::{JoinRequestStatus, Language};
use crate::error::AppError;

use super::{TelegramApi, TeloxideApi};

#[derive(Debug, Clone)]
pub struct StartInput {
    pub user_chat_id: i64,
    pub telegram_user_id: i64,
    pub first_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct PendingContactJoinRequest {
    join_request_id: i64,
    community_title: String,
    first_question_text: String,
    updated_at: DateTime<Utc>,
}

pub async fn handle_start(bot: Bot, msg: Message, pool: PgPool) -> Result<(), AppError> {
    let Some(from) = msg.from.as_ref() else {
        tracing::warn!("received /start without sender");
        return Ok(());
    };

    let input = StartInput {
        user_chat_id: msg.chat.id.0,
        telegram_user_id: from.id.0 as i64,
        first_name: from.first_name.clone(),
    };

    let api = TeloxideApi::new(bot);
    process_start(&api, &pool, input).await
}

pub async fn process_start(
    api: &dyn TelegramApi,
    pool: &PgPool,
    input: StartInput,
) -> Result<(), AppError> {
    let pending = find_pending_contact_request(pool, input.telegram_user_id).await?;

    let Some(pending) = pending else {
        api.send_message(
            input.user_chat_id,
            "Hi! If you've requested to join a community, I'll message you with some questions."
                .to_string(),
        )
        .await
        .map_err(|err| AppError::Telegram(err.to_string()))?;
        return Ok(());
    };

    let message = format!(
        "Hi {}! I saw your request to join {}.\n\nBefore a moderator reviews it, please answer a few quick questions.\n\n{}",
        input.first_name, pending.community_title, pending.first_question_text
    );

    api.send_message(input.user_chat_id, message)
        .await
        .map_err(|err| AppError::Telegram(err.to_string()))?;

    if SessionRepo::find_active_by_join_request_id(pool, pending.join_request_id)
        .await?
        .is_none()
    {
        SessionRepo::create(pool, pending.join_request_id, 1, Language::English).await?;
    }

    let _updated = JoinRequestRepo::update_status(
        pool,
        pending.join_request_id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::QuestionnaireInProgress,
        pending.updated_at,
    )
    .await?;

    tracing::info!(
        join_request_id = pending.join_request_id,
        telegram_user_id = input.telegram_user_id,
        "resumed questionnaire from /start"
    );

    Ok(())
}

async fn find_pending_contact_request(
    pool: &PgPool,
    telegram_user_id: i64,
) -> Result<Option<PendingContactJoinRequest>, AppError> {
    let row = sqlx::query_as::<_, PendingContactJoinRequest>(
        r#"SELECT jr.id AS join_request_id,
                  c.title AS community_title,
                  cq.question_text AS first_question_text,
                  jr.updated_at
           FROM join_requests jr
           INNER JOIN applicants a ON a.id = jr.applicant_id
           INNER JOIN communities c ON c.id = jr.community_id
           INNER JOIN community_questions cq
               ON cq.community_id = c.id
              AND cq.position = 1
              AND cq.is_active = TRUE
           WHERE a.telegram_user_id = $1
             AND jr.status = 'pending_contact'
           ORDER BY jr.created_at DESC
           LIMIT 1"#,
    )
    .bind(telegram_user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
