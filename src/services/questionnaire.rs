use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::fmt;

use crate::db::{AnswerRepo, CommunityRepo, JoinRequestRepo, SessionRepo};
use crate::domain::{
    ApplicantSession, CommunityQuestion, JoinRequest, JoinRequestStatus, SessionState,
};
use crate::error::AppError;

const LOW_EFFORT_ANSWERS: &[&str] = &[
    ".", "..", "x", "xx", "test", "asdf", "123", "aaa", "-", "no", "n/a",
];

const REQUIRED_ERROR_MESSAGE: &str = "This question is required. Please provide an answer.";
const TOO_SHORT_ERROR_MESSAGE: &str =
    "Please provide a more detailed answer (at least a few words).";
const LOW_EFFORT_ERROR_MESSAGE: &str =
    "Please provide a genuine answer so moderators can review your application.";

#[derive(Debug, Clone)]
pub enum AnswerValidationError {
    Required,
    TooShort,
    LowEffort,
}

impl AnswerValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Required => REQUIRED_ERROR_MESSAGE,
            Self::TooShort => TOO_SHORT_ERROR_MESSAGE,
            Self::LowEffort => LOW_EFFORT_ERROR_MESSAGE,
        }
    }
}

impl fmt::Display for AnswerValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for AnswerValidationError {}

#[derive(Debug, Clone)]
pub enum QuestionnaireStep {
    NextQuestion { question: CommunityQuestion },
    Completed { join_request: JoinRequest },
}

#[derive(Debug, Clone)]
pub enum ProcessAnswerResult {
    ValidationFailed { message: &'static str },
    Advanced { step: QuestionnaireStep },
}

#[derive(Debug, Clone)]
pub struct ActiveQuestionnaireContext {
    pub join_request: JoinRequest,
    pub session: ApplicantSession,
    pub current_question: CommunityQuestion,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ActiveQuestionnaireRow {
    join_request_id: i64,
    join_request_community_id: i64,
    join_request_applicant_id: i64,
    join_request_telegram_user_chat_id: i64,
    join_request_status: JoinRequestStatus,
    join_request_telegram_join_request_date: DateTime<Utc>,
    join_request_submitted_to_moderators_at: Option<DateTime<Utc>>,
    join_request_approved_at: Option<DateTime<Utc>>,
    join_request_rejected_at: Option<DateTime<Utc>>,
    join_request_moderator_message_chat_id: Option<i64>,
    join_request_moderator_message_id: Option<i64>,
    join_request_created_at: DateTime<Utc>,
    join_request_updated_at: DateTime<Utc>,
    join_request_reminder_sent_at: Option<DateTime<Utc>>,
    session_id: i64,
    session_join_request_id: i64,
    session_current_question_position: i32,
    session_state: SessionState,
    session_created_at: DateTime<Utc>,
    session_updated_at: DateTime<Utc>,
    question_id: i64,
    question_community_id: i64,
    question_key: String,
    question_text: String,
    question_required: bool,
    question_position: i32,
    question_is_active: bool,
    question_created_at: DateTime<Utc>,
    question_updated_at: DateTime<Utc>,
}

pub fn validate_answer(
    answer: &str,
    required: bool,
) -> Result<String, AnswerValidationError> {
    let trimmed = answer.trim();

    if trimmed.is_empty() {
        return if required {
            Err(AnswerValidationError::Required)
        } else {
            Ok(String::new())
        };
    }

    if required && trimmed.chars().count() < 2 {
        return Err(AnswerValidationError::TooShort);
    }

    let lowered = trimmed.to_lowercase();
    if LOW_EFFORT_ANSWERS.contains(&lowered.as_str()) {
        return Err(AnswerValidationError::LowEffort);
    }

    Ok(trimmed.to_string())
}

pub async fn find_active_context_by_telegram_user_id(
    pool: &PgPool,
    telegram_user_id: i64,
) -> Result<Option<ActiveQuestionnaireContext>, AppError> {
    let row = sqlx::query_as::<_, ActiveQuestionnaireRow>(
        r#"SELECT
                jr.id AS join_request_id,
                jr.community_id AS join_request_community_id,
                jr.applicant_id AS join_request_applicant_id,
                jr.telegram_user_chat_id AS join_request_telegram_user_chat_id,
                jr.status AS join_request_status,
                jr.telegram_join_request_date AS join_request_telegram_join_request_date,
                jr.submitted_to_moderators_at AS join_request_submitted_to_moderators_at,
                jr.approved_at AS join_request_approved_at,
                jr.rejected_at AS join_request_rejected_at,
                jr.moderator_message_chat_id AS join_request_moderator_message_chat_id,
                jr.moderator_message_id AS join_request_moderator_message_id,
                jr.created_at AS join_request_created_at,
                jr.updated_at AS join_request_updated_at,
                jr.reminder_sent_at AS join_request_reminder_sent_at,
                s.id AS session_id,
                s.join_request_id AS session_join_request_id,
                s.current_question_position AS session_current_question_position,
                s.state AS session_state,
                s.created_at AS session_created_at,
                s.updated_at AS session_updated_at,
                q.id AS question_id,
                q.community_id AS question_community_id,
                q.question_key AS question_key,
                q.question_text AS question_text,
                q.required AS question_required,
                q.position AS question_position,
                q.is_active AS question_is_active,
                q.created_at AS question_created_at,
                q.updated_at AS question_updated_at
            FROM applicants a
            INNER JOIN join_requests jr
                ON jr.applicant_id = a.id
            INNER JOIN applicant_sessions s
                ON s.join_request_id = jr.id
               AND s.state = 'awaiting_answer'
            INNER JOIN community_questions q
                ON q.community_id = jr.community_id
               AND q.position = s.current_question_position
               AND q.is_active = TRUE
            WHERE a.telegram_user_id = $1
              AND jr.status = 'questionnaire_in_progress'
            ORDER BY jr.created_at DESC
            LIMIT 1"#,
    )
    .bind(telegram_user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| ActiveQuestionnaireContext {
        join_request: JoinRequest {
            id: row.join_request_id,
            community_id: row.join_request_community_id,
            applicant_id: row.join_request_applicant_id,
            telegram_user_chat_id: row.join_request_telegram_user_chat_id,
            status: row.join_request_status,
            telegram_join_request_date: row.join_request_telegram_join_request_date,
            submitted_to_moderators_at: row.join_request_submitted_to_moderators_at,
            approved_at: row.join_request_approved_at,
            rejected_at: row.join_request_rejected_at,
            moderator_message_chat_id: row.join_request_moderator_message_chat_id,
            moderator_message_id: row.join_request_moderator_message_id,
            created_at: row.join_request_created_at,
            updated_at: row.join_request_updated_at,
            reminder_sent_at: row.join_request_reminder_sent_at,
        },
        session: ApplicantSession {
            id: row.session_id,
            join_request_id: row.session_join_request_id,
            current_question_position: row.session_current_question_position,
            state: row.session_state,
            created_at: row.session_created_at,
            updated_at: row.session_updated_at,
        },
        current_question: CommunityQuestion {
            id: row.question_id,
            community_id: row.question_community_id,
            question_key: row.question_key,
            question_text: row.question_text,
            required: row.question_required,
            position: row.question_position,
            is_active: row.question_is_active,
            created_at: row.question_created_at,
            updated_at: row.question_updated_at,
        },
    }))
}

pub async fn process_answer(
    pool: &PgPool,
    context: ActiveQuestionnaireContext,
    answer_text: &str,
) -> Result<ProcessAnswerResult, AppError> {
    let validated_answer = match validate_answer(answer_text, context.current_question.required) {
        Ok(answer) => answer,
        Err(err) => {
            return Ok(ProcessAnswerResult::ValidationFailed {
                message: err.message(),
            });
        }
    };

    AnswerRepo::create(
        pool,
        context.join_request.id,
        context.current_question.id,
        &validated_answer,
    )
    .await?;

    let questions = CommunityRepo::find_active_questions(pool, context.join_request.community_id).await?;
    let next_question = questions
        .into_iter()
        .find(|q| q.position > context.session.current_question_position);

    if let Some(next_question) = next_question {
        SessionRepo::advance_question(pool, context.session.id, next_question.position).await?;
        return Ok(ProcessAnswerResult::Advanced {
            step: QuestionnaireStep::NextQuestion {
                question: next_question,
            },
        });
    }

    SessionRepo::complete(pool, context.session.id).await?;
    let updated_join_request = JoinRequestRepo::update_status(
        pool,
        context.join_request.id,
        JoinRequestStatus::QuestionnaireInProgress,
        JoinRequestStatus::Submitted,
        context.join_request.updated_at,
    )
    .await?;

    Ok(ProcessAnswerResult::Advanced {
        step: QuestionnaireStep::Completed {
            join_request: updated_join_request,
        },
    })
}
