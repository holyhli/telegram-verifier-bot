use std::sync::Arc;

use sqlx::PgPool;
use teloxide::prelude::Bot;
use teloxide::types::Message;

use crate::config::Config;
use crate::error::AppError;
use crate::messages::Messages;
use crate::services::moderator::send_moderator_card;
use crate::db::{ApplicantRepo, JoinRequestRepo};
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
    // Check if user has a pending_contact join request (waiting for name reply).
    // The user replied to our bilingual name prompt — store their name and send the language keyboard.
    // Now that they've messaged us, we have permission to send inline keyboards.
    if let Some(pending_jr) = JoinRequestRepo::find_pending_contact_by_telegram_user_id(pool, input.telegram_user_id).await? {
        // Update the applicant's first_name with what they actually typed
        let typed_name = input.text.trim().to_string();
        let applicant = sqlx::query!(
            "SELECT id FROM applicants WHERE id = (SELECT applicant_id FROM join_requests WHERE id = $1)",
            pending_jr.id
        )
        .fetch_one(pool)
        .await?;
        ApplicantRepo::update_profile(pool, applicant.id, &typed_name, None, None).await?;

        tracing::info!(
            join_request_id = pending_jr.id,
            telegram_user_id = input.telegram_user_id,
            name = %typed_name,
            "name reply received, sending language selection keyboard"
        );
        let keyboard = vec![vec![
            ("\u{1F1EC}\u{1F1E7} English".to_string(), "lang:en".to_string()),
            ("\u{1F1FA}\u{1F1E6} \u{0423}\u{043a}\u{0440}\u{0430}\u{0457}\u{043d}\u{0441}\u{044c}\u{043a}\u{0430}".to_string(), "lang:uk".to_string()),
        ]];
        let prompt = Messages::language_selection_prompt();
        api.send_message_with_inline_keyboard(input.chat_id, prompt, keyboard)
            .await
            .map_err(|e| AppError::Telegram(e.to_string()))?;
        return Ok(());
    }

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
