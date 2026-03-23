use sqlx::PgPool;

use crate::db::{AnswerRepo, CommunityRepo, JoinRequestRepo, QuestionEventRepo, SessionRepo};
use crate::domain::{JoinRequestStatus, Language, QuestionEventType};
use crate::error::AppError;
use crate::messages::Messages;

use super::join_request::is_user_unreachable_error;
use super::TelegramApi;

/// Processes language selection callback from inline keyboard.
///
/// Flow:
/// 1. Parse callback data (lang:en or lang:uk)
/// 2. Validate language code
/// 3. Load join request by telegram_user_id and user_chat_id
/// 4. Validate join request status is PendingContact
/// 5. Load first question for the community
/// 6. Load community for title
/// 7. Load applicant for first name
/// 8. Build message text
/// 9. Send message FIRST (before creating session)
/// 10. If message sent successfully: create session, update status, answer callback
/// 11. If user unreachable: log warning, answer callback with neutral message, return Ok
/// 12. If other error: return Err
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

    // 4. Load all questions for this community
    let all_questions = CommunityRepo::find_active_questions(pool, join_request.community_id).await?;
    let first_question = all_questions.iter()
        .find(|q| q.position == 1)
        .ok_or_else(|| AppError::Internal("No question at position 1".into()))?;
    let second_question = all_questions.iter()
        .find(|q| q.position == 2);

    // 5. Load community for title
    let community = CommunityRepo::find_by_id(pool, join_request.community_id)
        .await?
        .ok_or_else(|| AppError::Internal("Community not found".into()))?;

    // 6. Load applicant (name was already updated when they replied to the name prompt)
    let applicant = sqlx::query!(
        "SELECT id, first_name FROM applicants WHERE id = $1",
        join_request.applicant_id
    )
    .fetch_one(pool)
    .await?;

    // 7. Build first message: welcome + question 2 (question 1 was already answered via name prompt)
    let welcome = Messages::welcome_message(&applicant.first_name, &community.title, language);
    let first_message_text = match second_question {
        Some(q) => format!("{}\n\n{}", welcome, q.text_for_language(language)),
        None => welcome.clone(),
    };

    // 8. Send message FIRST (before writing to DB)
    // At this point the user has already messaged us, so we have full send permission.
    let send_chat_id = join_request.telegram_user_chat_id;
    match api.send_message(send_chat_id, first_message_text).await {
        Ok(_) => {
            // Store the name (question 1) answer using the applicant's saved first_name.
            // The user typed their name earlier; it was saved to applicants.first_name.
            AnswerRepo::create(pool, join_request.id, first_question.id, &applicant.first_name).await?;

            // Record question_presented + answer_accepted for Q1 (name question).
            // The name was collected before language selection; we emit both events now
            // so that compute_per_question_timing can calculate a duration for Q1.
            if let Err(e) = QuestionEventRepo::create(pool, join_request.id, first_question.id, join_request.applicant_id, QuestionEventType::QuestionPresented, None).await {
                tracing::error!(join_request_id = join_request.id, error = %e, "failed to record question_presented event for Q1");
            }
            if let Err(e) = QuestionEventRepo::create(pool, join_request.id, first_question.id, join_request.applicant_id, QuestionEventType::AnswerAccepted, None).await {
                tracing::error!(join_request_id = join_request.id, error = %e, "failed to record answer_accepted event for Q1");
            }

            // Create session starting at position 2 (question 1 already answered).
            let start_position = if second_question.is_some() { 2 } else { 1 };
            SessionRepo::create(pool, join_request.id, start_position, language).await?;

            JoinRequestRepo::update_status(
                pool,
                join_request.id,
                JoinRequestStatus::PendingContact,
                JoinRequestStatus::QuestionnaireInProgress,
                join_request.updated_at,
            )
            .await?;

            tracing::info!(
                join_request_id = join_request.id,
                community_id = community.id,
                telegram_user_id = telegram_user_id,
                language = ?language,
                "language selected, questionnaire started at question 2"
            );

            if let Some(q) = second_question {
                if let Err(e) = QuestionEventRepo::create(pool, join_request.id, q.id, join_request.applicant_id, QuestionEventType::QuestionPresented, None).await {
                    tracing::error!(join_request_id = join_request.id, error = %e, "failed to record question_presented event");
                }
            }

            // Answer callback query with confirmation
            let confirmation = match language {
                Language::English => "Language set to English",
                Language::Ukrainian => "\u{041c}\u{043e}\u{0432}\u{0443} \u{0432}\u{0441}\u{0442}\u{0430}\u{043d}\u{043e}\u{0432}\u{043b}\u{0435}\u{043d}\u{043e}: \u{0423}\u{043a}\u{0440}\u{0430}\u{0457}\u{043d}\u{0441}\u{044c}\u{043a}\u{0430}",
            };
            api.answer_callback_query(callback_query_id, confirmation.to_string())
                .await
                .map_err(|e| AppError::Telegram(e.to_string()))?;

            Ok(())
        }
        Err(err) if is_user_unreachable_error(&err) => {
            // User is unreachable (blocked, deactivated, etc.)
            tracing::warn!(
                join_request_id = join_request.id,
                community_id = community.id,
                telegram_user_id = telegram_user_id,
                error = %err,
                "applicant cannot be contacted, language selection cancelled"
            );

            // Still answer the callback query with a neutral message
            api.answer_callback_query(
                callback_query_id,
                "Could not reach you. Please try again later.".to_string(),
            )
            .await
            .map_err(|e| AppError::Telegram(e.to_string()))?;

            Ok(())
        }
        Err(err) => {
            // Other telegram errors
            tracing::error!(
                join_request_id = join_request.id,
                community_id = community.id,
                telegram_user_id = telegram_user_id,
                error = %err,
                "failed to send welcome message"
            );
            Err(AppError::Telegram(err.to_string()))
        }
    }
}
