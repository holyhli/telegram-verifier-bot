use chrono::{DateTime, Utc};

/// Type of event that occurred during question interaction.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum QuestionEventType {
    QuestionPresented,
    ValidationFailed,
    AnswerAccepted,
}

/// An event recording an interaction with a question during the questionnaire flow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct QuestionEvent {
    pub id: i64,
    pub join_request_id: i64,
    pub community_question_id: i64,
    pub applicant_id: i64,
    pub event_type: QuestionEventType,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
