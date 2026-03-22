use chrono::{DateTime, Utc};
use std::fmt;

/// The type of moderation action taken on a join request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Approved,
    Rejected,
    Banned,
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::Banned => write!(f, "banned"),
        }
    }
}

/// A moderation action recorded when a moderator acts on a join request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ModerationAction {
    pub id: i64,
    pub join_request_id: i64,
    pub moderator_telegram_user_id: i64,
    pub action_type: ActionType,
    pub created_at: DateTime<Utc>,
}
