use sqlx::PgPool;

use crate::db::{CommunityRepo, JoinRequestRepo, SessionRepo};
use crate::domain::{JoinRequestStatus, Language};
use crate::error::AppError;
use crate::messages::Messages;

use super::TelegramApi;

/// Processes language selection callback from inline keyboard.
///
/// Flow:
/// 1. Parse callback data (lang:en or lang:uk)
/// 2. Validate language code
/// 3. Load join request by telegram_user_id and user_chat_id
/// 4. Validate join request status is PendingContact
/// 5. Load first question for the community
/// 6. Create session with selected language at position 1
/// 7. Load community for title
/// 8. Send welcome message + first question in selected language
/// 9. Transition join request to QuestionnaireInProgress
/// 10. Answer callback query with confirmation
pub async fn process_language_selection_callback(
    api: &dyn TelegramApi,
    pool: &PgPool,
    callback_query_id: String,
    telegram_user_id: i64,
    user_chat_id: i64,
    callback_data: String,
) -> Result<(), AppError> {
    // 1. Parse language code
    let lang_code = callback_data
        .strip_prefix("lang:")
        .ok_or_else(|| AppError::Internal("Invalid callback data format".into()))?;

    let language = Language::from_code(lang_code)
        .ok_or_else(|| AppError::Internal(format!("Unknown language code: {}", lang_code)))?;

    // 2. Load join request
    let join_request =
        JoinRequestRepo::find_active_by_telegram_user_id_and_chat_id(pool, telegram_user_id, user_chat_id)
            .await?
            .ok_or_else(|| AppError::NotFound("No active join request found".into()))?;

    // 3. Validate status
    if join_request.status != JoinRequestStatus::PendingContact {
        return Err(AppError::InvalidStateTransition {
            from: join_request.status,
            to: JoinRequestStatus::QuestionnaireInProgress,
        });
    }

    // 4. Load first question
    let first_question = CommunityRepo::find_active_questions(pool, join_request.community_id)
        .await?
        .into_iter()
        .find(|q| q.position == 1)
        .ok_or_else(|| AppError::Internal("No question at position 1".into()))?;

    // 5. Create session with language
    SessionRepo::create(pool, join_request.id, 1, language).await?;

    // 6. Load community for title
    let community = CommunityRepo::find_by_id(pool, join_request.community_id)
        .await?
        .ok_or_else(|| AppError::Internal("Community not found".into()))?;

    // 7. Load applicant for first name
    let applicant = sqlx::query!(
        "SELECT first_name FROM applicants WHERE id = $1",
        join_request.applicant_id
    )
    .fetch_one(pool)
    .await?;

    // 8. Send welcome + first question
    let welcome = Messages::welcome_message(&applicant.first_name, &community.title, language);
    let question_text = first_question.text_for_language(language);
    let full_message = format!("{}\n\n{}", welcome, question_text);

    api.send_message(user_chat_id, full_message)
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?;

    // 9. Transition status
    JoinRequestRepo::update_status(
        pool,
        join_request.id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::QuestionnaireInProgress,
        join_request.updated_at,
    )
    .await?;

    // 10. Answer callback
    let confirmation = match language {
        Language::English => "Language set to English",
        Language::Ukrainian => "Мову встановлено: Українська",
    };
    api.answer_callback_query(callback_query_id, confirmation.to_string())
        .await
        .map_err(|e| AppError::Telegram(e.to_string()))?;

    Ok(())
}
