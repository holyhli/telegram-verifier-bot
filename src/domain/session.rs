use chrono::{DateTime, Utc};
use std::fmt;

/// The state of an applicant's questionnaire session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    AwaitingAnswer,
    Completed,
    Expired,
    Cancelled,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AwaitingAnswer => write!(f, "awaiting_answer"),
            Self::Completed => write!(f, "completed"),
            Self::Expired => write!(f, "expired"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Tracks the progress of an applicant answering community questions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ApplicantSession {
    pub id: i64,
    pub join_request_id: i64,
    pub current_question_position: i32,
    pub state: SessionState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
