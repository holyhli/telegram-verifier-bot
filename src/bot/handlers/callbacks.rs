use std::sync::Arc;

use chrono::Utc;
use sqlx::PgPool;
use teloxide::prelude::Bot;
use teloxide::types::CallbackQuery;
use teloxide::{ApiError, RequestError};

use crate::config::Config;
use crate::db::{BlacklistRepo, JoinRequestRepo, ModerationActionRepo};
use crate::domain::{ActionType, JoinRequestStatus, ScopeType};
use crate::error::AppError;
use crate::services::moderator::{
    load_moderator_card_answers, load_moderator_card_context, render_moderator_card,
};

use super::{language_selection, TelegramApi, TeloxideApi};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAction {
    Approve,
    Reject,
    Ban,
}

#[derive(Debug, Clone)]
pub struct CallbackActionInput {
    pub callback_query_id: String,
    pub callback_data: Option<String>,
    pub moderator_telegram_user_id: i64,
    pub message_chat_id: Option<i64>,
    pub message_id: Option<i64>,
}

pub fn parse_callback_data(value: &str) -> Option<(CallbackAction, i64)> {
    let (action, id) = value.split_once(':')?;
    let join_request_id = id.parse::<i64>().ok()?;

    let action = match action {
        "a" => CallbackAction::Approve,
        "r" => CallbackAction::Reject,
        "b" => CallbackAction::Ban,
        _ => return None,
    };

    Some((action, join_request_id))
}

pub async fn handle_callback_query(
    bot: Bot,
    query: CallbackQuery,
    pool: PgPool,
    config: Arc<Config>,
) -> Result<(), AppError> {
    let callback_data = query.data.clone();
    let api = TeloxideApi::new(bot);

    if let Some(ref data) = callback_data {
        if data.starts_with("lang:") {
            let telegram_user_id = query.from.id.0 as i64;
            let user_chat_id = query.from.id.0 as i64;
            return language_selection::process_language_selection_callback(
                &api,
                &pool,
                query.id.to_string(),
                telegram_user_id,
                user_chat_id,
                data.clone(),
            )
            .await;
        }
    }

    let regular_message = query.regular_message().cloned();
    let moderator_id = query.from.id.0 as i64;
    let input = CallbackActionInput {
        callback_query_id: query.id.to_string(),
        callback_data,
        moderator_telegram_user_id: moderator_id,
        message_chat_id: regular_message.as_ref().map(|message| message.chat.id.0),
        message_id: regular_message.as_ref().map(|message| message.id.0 as i64),
    };

    process_callback_query(&api, &pool, &config, input).await
}

pub async fn process_callback_query(
    api: &dyn TelegramApi,
    pool: &PgPool,
    config: &Config,
    input: CallbackActionInput,
) -> Result<(), AppError> {
    let callback_data = match input.callback_data.as_deref() {
        Some(data) => data,
        None => {
            answer_callback(api, &input.callback_query_id, "Invalid action payload").await;
            return Ok(());
        }
    };

    let (action, join_request_id) = match parse_callback_data(callback_data) {
        Some(parsed) => parsed,
        None => {
            answer_callback(api, &input.callback_query_id, "Invalid action payload").await;
            return Ok(());
        }
    };

    if !config
        .allowed_moderator_ids
        .contains(&input.moderator_telegram_user_id)
    {
        answer_callback(api, &input.callback_query_id, "You are not authorized").await;
        return Ok(());
    }

    let context = load_moderator_card_context(pool, join_request_id).await?;
    let current = JoinRequestRepo::find_by_id(pool, join_request_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("join request {join_request_id}")))?;

    if current.status != JoinRequestStatus::Submitted {
        answer_callback(
            api,
            &input.callback_query_id,
            "Already processed by another moderator",
        )
        .await;
        return Ok(());
    }

    let next_status = match action {
        CallbackAction::Approve => JoinRequestStatus::Approved,
        CallbackAction::Reject => JoinRequestStatus::Rejected,
        CallbackAction::Ban => JoinRequestStatus::Banned,
    };

    match JoinRequestRepo::update_status(
        pool,
        join_request_id,
        JoinRequestStatus::Submitted,
        next_status,
        current.updated_at,
    )
    .await
    {
        Ok(_) => {}
        Err(AppError::AlreadyProcessed { .. }) => {
            answer_callback(
                api,
                &input.callback_query_id,
                "Already processed by another moderator",
            )
            .await;
            return Ok(());
        }
        Err(err) => return Err(err),
    }

    if let Err(err) = execute_telegram_decision(
        api,
        action,
        context.community_chat_id,
        context.applicant_telegram_user_id,
    )
    .await
    {
        if is_hide_requester_missing_error(&err) {
            answer_callback(
                api,
                &input.callback_query_id,
                "Request already processed outside bot",
            )
            .await;
            return Ok(());
        }

        return Err(AppError::Telegram(err.to_string()));
    }

    record_decision_timestamp(pool, join_request_id, action).await?;

    if action == CallbackAction::Ban {
        BlacklistRepo::create(
            pool,
            context.applicant_telegram_user_id,
            ScopeType::Community,
            Some(current.community_id),
            Some("Banned from community by moderator action"),
            input.moderator_telegram_user_id,
        )
        .await?;
    }

    let action_type = match action {
        CallbackAction::Approve => ActionType::Approved,
        CallbackAction::Reject => ActionType::Rejected,
        CallbackAction::Ban => ActionType::Banned,
    };
    ModerationActionRepo::create(
        pool,
        join_request_id,
        input.moderator_telegram_user_id,
        action_type,
    )
    .await?;

    if let Err(err) = api
        .send_message(
            context.applicant_chat_id,
            applicant_decision_message(action).to_string(),
        )
        .await
    {
        if !is_user_unreachable_error(&err) {
            return Err(AppError::Telegram(err.to_string()));
        }
    }

    let answers = load_moderator_card_answers(pool, join_request_id).await?;
    let mut card_text = render_moderator_card(&context, &answers);
    card_text = card_text.replace(
        "<b>Status:</b> Submitted",
        &format!("<b>Status:</b> {}", action_status_label(action)),
    );
    card_text.push_str(&format!(
        "\n\n<b>Action:</b> {}\n<b>Moderator:</b> <code>{}</code>\n<b>Processed at:</b> {} UTC",
        action_status_label(action),
        input.moderator_telegram_user_id,
        Utc::now().format("%Y-%m-%d %H:%M:%S")
    ));

    let message_chat_id = input
        .message_chat_id
        .or(current.moderator_message_chat_id)
        .unwrap_or(config.default_moderator_chat_id);
    let message_id = input.message_id.or(current.moderator_message_id).unwrap_or(0);

    if message_id > 0 {
        api.edit_message_html(message_chat_id, message_id, card_text)
            .await
            .map_err(|err| AppError::Telegram(err.to_string()))?;
        api.clear_message_reply_markup(message_chat_id, message_id)
            .await
            .map_err(|err| AppError::Telegram(err.to_string()))?;
    }

    answer_callback(
        api,
        &input.callback_query_id,
        &format!("Request {}", action_status_label(action).to_lowercase()),
    )
    .await;

    Ok(())
}

async fn execute_telegram_decision(
    api: &dyn TelegramApi,
    action: CallbackAction,
    community_chat_id: i64,
    applicant_telegram_user_id: i64,
) -> Result<(), RequestError> {
    let result = match action {
        CallbackAction::Approve => {
            api.approve_chat_join_request(community_chat_id, applicant_telegram_user_id)
                .await
        }
        CallbackAction::Reject | CallbackAction::Ban => {
            api.decline_chat_join_request(community_chat_id, applicant_telegram_user_id)
                .await
        }
    };

    result
}

async fn record_decision_timestamp(
    pool: &PgPool,
    join_request_id: i64,
    action: CallbackAction,
) -> Result<(), AppError> {
    match action {
        CallbackAction::Approve => {
            sqlx::query("UPDATE join_requests SET approved_at = NOW(), updated_at = NOW() WHERE id = $1")
                .bind(join_request_id)
                .execute(pool)
                .await?;
        }
        CallbackAction::Reject | CallbackAction::Ban => {
            sqlx::query("UPDATE join_requests SET rejected_at = NOW(), updated_at = NOW() WHERE id = $1")
                .bind(join_request_id)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

async fn answer_callback(api: &dyn TelegramApi, callback_query_id: &str, text: &str) {
    if let Err(err) = api
        .answer_callback_query(callback_query_id.to_string(), text.to_string())
        .await
    {
        tracing::warn!(callback_query_id, error = %err, "failed to answer callback query");
    }
}

fn action_status_label(action: CallbackAction) -> &'static str {
    match action {
        CallbackAction::Approve => "Approved",
        CallbackAction::Reject => "Rejected",
        CallbackAction::Ban => "Banned",
    }
}

fn applicant_decision_message(action: CallbackAction) -> &'static str {
    match action {
        CallbackAction::Approve => {
            "Your request has been approved. Welcome to the community!"
        }
        CallbackAction::Reject => {
            "Your join request was reviewed and declined by the moderators."
        }
        CallbackAction::Ban => {
            "Your join request was declined and you have been banned by the moderators."
        }
    }
}

fn is_hide_requester_missing_error(err: &RequestError) -> bool {
    matches!(
        err,
        RequestError::Api(ApiError::Unknown(details)) if details.contains("HIDE_REQUESTER_MISSING")
    )
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
