use chrono::{DateTime, Utc};

/// A Telegram user who has submitted a join request to a community.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Applicant {
    pub id: i64,
    pub telegram_user_id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
