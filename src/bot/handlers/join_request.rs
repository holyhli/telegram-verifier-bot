use chrono::{DateTime, Utc};
use sqlx::PgPool;
use teloxide::prelude::Bot;
use teloxide::types::ChatJoinRequest;
use teloxide::ApiError;
use teloxide::RequestError;

use crate::db::{ApplicantRepo, BlacklistRepo, CommunityRepo, JoinRequestRepo, SessionRepo};
use crate::domain::{JoinRequestStatus, ScopeType};
use crate::error::AppError;

use super::{TelegramApi, TeloxideApi};

#[derive(Debug, Clone)]
pub struct JoinRequestInput {
    pub community_chat_id: i64,
    pub community_title: String,
    pub telegram_user_id: i64,
    pub user_chat_id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub join_request_date: DateTime<Utc>,
}

pub async fn handle_join_request(
    bot: Bot,
    join_request: ChatJoinRequest,
    pool: PgPool,
) -> Result<(), AppError> {
    let input = JoinRequestInput {
        community_chat_id: join_request.chat.id.0,
        community_title: join_request.chat.title().unwrap_or("community").to_string(),
        telegram_user_id: join_request.from.id.0 as i64,
        user_chat_id: join_request.user_chat_id.0,
        first_name: join_request.from.first_name,
        last_name: join_request.from.last_name,
        username: join_request.from.username,
        join_request_date: join_request.date,
    };

    let api = TeloxideApi::new(bot);
    process_join_request(&api, &pool, input).await
}

pub async fn process_join_request(
    api: &dyn TelegramApi,
    pool: &PgPool,
    input: JoinRequestInput,
) -> Result<(), AppError> {
    let Some(community) =
        CommunityRepo::find_by_telegram_chat_id(pool, input.community_chat_id).await?
    else {
        tracing::warn!(
            community_chat_id = input.community_chat_id,
            telegram_user_id = input.telegram_user_id,
            "join request received for unknown community"
        );
        return Ok(());
    };

    let blacklist_entries =
        BlacklistRepo::find_by_telegram_user_id(pool, input.telegram_user_id).await?;
    let is_blacklisted = blacklist_entries.iter().any(|entry| {
        entry.scope_type == ScopeType::Global || entry.community_id == Some(community.id)
    });

    if is_blacklisted {
        match api
            .decline_chat_join_request(input.community_chat_id, input.telegram_user_id)
            .await
        {
            Ok(()) => {
                tracing::info!(
                    community_id = community.id,
                    telegram_user_id = input.telegram_user_id,
                    "declined blacklisted join request"
                );
            }
            Err(err) => {
                tracing::error!(
                    community_id = community.id,
                    telegram_user_id = input.telegram_user_id,
                    error = %err,
                    "failed to decline blacklisted join request"
                );
            }
        }
        return Ok(());
    }

    let applicant = ApplicantRepo::find_or_create_by_telegram_user_id(
        pool,
        input.telegram_user_id,
        &input.first_name,
        input.last_name.as_deref(),
        input.username.as_deref(),
    )
    .await?;

    let existing =
        JoinRequestRepo::find_active_for_applicant_in_community(pool, applicant.id, community.id)
            .await?;
    if existing.is_some() {
        tracing::info!(
            community_id = community.id,
            applicant_id = applicant.id,
            telegram_user_id = applicant.telegram_user_id,
            "duplicate join request update ignored"
        );
        return Ok(());
    }

    let join_request = JoinRequestRepo::create(
        pool,
        community.id,
        applicant.id,
        input.user_chat_id,
        input.join_request_date,
    )
    .await?;

    let first_question = CommunityRepo::find_active_questions(pool, community.id)
        .await?
        .into_iter()
        .find(|q| q.position == 1)
        .ok_or_else(|| {
            AppError::Internal(format!(
                "community {} has no active question at position 1",
                community.id
            ))
        })?;

    let text = format!(
        "Hi {}! I saw your request to join {}.\n\nBefore a moderator reviews it, please answer a few quick questions.\n\n{}",
        input.first_name, input.community_title, first_question.question_text
    );

    match api.send_message(input.user_chat_id, text).await {
        Ok(()) => {
            SessionRepo::create(pool, join_request.id, 1).await?;
            let updated = JoinRequestRepo::update_status(
                pool,
                join_request.id,
                JoinRequestStatus::PendingContact,
                JoinRequestStatus::QuestionnaireInProgress,
                join_request.updated_at,
            )
            .await?;

            tracing::info!(
                join_request_id = updated.id,
                community_id = updated.community_id,
                telegram_user_id = applicant.telegram_user_id,
                "join request processed"
            );

            Ok(())
        }
        Err(err) if is_user_unreachable_error(&err) => {
            let _ = JoinRequestRepo::update_status(
                pool,
                join_request.id,
                JoinRequestStatus::PendingContact,
                JoinRequestStatus::Cancelled,
                join_request.updated_at,
            )
            .await;

            tracing::warn!(
                join_request_id = join_request.id,
                community_id = community.id,
                telegram_user_id = applicant.telegram_user_id,
                error = %err,
                "applicant cannot be contacted, join request cancelled"
            );
            Ok(())
        }
        Err(err) => {
            tracing::error!(
                join_request_id = join_request.id,
                community_id = community.id,
                telegram_user_id = applicant.telegram_user_id,
                error = %err,
                "failed to send first questionnaire message"
            );
            Ok(())
        }
    }
}

fn is_user_unreachable_error(err: &RequestError) -> bool {
    match err {
        RequestError::Api(ApiError::BotBlocked)
        | RequestError::Api(ApiError::UserDeactivated)
        | RequestError::Api(ApiError::ChatNotFound) => true,
        RequestError::Api(ApiError::Unknown(details)) => {
            details.contains("blocked by the user")
                || details.contains("blocked by user")
                || details.contains("Forbidden")
        }
        _ => false,
    }
}
