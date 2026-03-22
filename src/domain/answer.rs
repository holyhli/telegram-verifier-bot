use chrono::{DateTime, Utc};

/// An answer provided by an applicant to a community question.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct JoinRequestAnswer {
    pub id: i64,
    pub join_request_id: i64,
    pub community_question_id: i64,
    pub answer_text: String,
    pub created_at: DateTime<Utc>,
}
