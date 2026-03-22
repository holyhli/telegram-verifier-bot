use std::sync::Arc;

use sqlx::PgPool;
use teloxide::prelude::Bot;
use teloxide::types::Message;

use crate::config::Config;
use crate::error::AppError;
use crate::messages::Messages;
use crate::services::moderator::send_moderator_card;
use crate::services::questionnaire::{
    find_active_context_by_telegram_user_id, process_answer, ProcessAnswerResult, QuestionnaireStep,
};

use super::{TelegramApi, TeloxideApi};

#[derive(Debug, Clone)]
pub struct PrivateMessageInput {
    pub chat_id: i64,
    pub telegram_user_id: i64,
    pub text: String,
}

pub async fn handle_private_message(
    bot: Bot,
    msg: Message,
    pool: PgPool,
    config: Arc<Config>,
) -> Result<(), AppError> {
    if !msg.chat.is_private() {
        return Ok(());
    }

    let Some(from) = msg.from.as_ref() else {
        tracing::warn!("received private message without sender");
        return Ok(());
    };

    let Some(text) = msg.text() else {
        return Ok(());
    };

    if text.trim_start().starts_with('/') {
        return Ok(());
    }

    let api = TeloxideApi::new(bot);
    let input = PrivateMessageInput {
        chat_id: msg.chat.id.0,
        telegram_user_id: from.id.0 as i64,
        text: text.to_string(),
    };

    process_private_message(&api, &pool, input, config.default_moderator_chat_id).await
}

pub async fn process_private_message(
    api: &dyn TelegramApi,
    pool: &PgPool,
    input: PrivateMessageInput,
    default_moderator_chat_id: i64,
) -> Result<(), AppError> {
    let Some(context) = find_active_context_by_telegram_user_id(pool, input.telegram_user_id).await?
    else {
        tracing::debug!(
            telegram_user_id = input.telegram_user_id,
            "private message ignored: no active questionnaire session"
        );
        return Ok(());
    };

    let language = context.session.language;
    let result = process_answer(pool, context, &input.text).await?;

    match result {
        ProcessAnswerResult::ValidationFailed { message } => {
            api.send_message(input.chat_id, message)
                .await
                .map_err(|err| AppError::Telegram(err.to_string()))?;
        }
        ProcessAnswerResult::Advanced {
            step: QuestionnaireStep::NextQuestion { question },
        } => {
            let question_text = question.text_for_language(language);
            api.send_message(input.chat_id, question_text.to_string())
                .await
                .map_err(|err| AppError::Telegram(err.to_string()))?;
        }
        ProcessAnswerResult::Advanced {
            step: QuestionnaireStep::Completed { join_request },
        } => {
            let completion_msg = Messages::completion_message(language);
            api.send_message(input.chat_id, completion_msg)
                .await
                .map_err(|err| AppError::Telegram(err.to_string()))?;

            let _updated = send_moderator_card(api, pool, join_request.id, default_moderator_chat_id)
                .await?;
        }
    }

    Ok(())
}
