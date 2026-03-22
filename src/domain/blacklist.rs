use chrono::{DateTime, Utc};
use std::fmt;

/// The scope of a blacklist entry — global or per-community.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ScopeType {
    Global,
    Community,
}

impl fmt::Display for ScopeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global => write!(f, "global"),
            Self::Community => write!(f, "community"),
        }
    }
}

/// A blacklist entry preventing a user from joining communities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct BlacklistEntry {
    pub id: i64,
    pub telegram_user_id: i64,
    pub scope_type: ScopeType,
    pub community_id: Option<i64>,
    pub reason: Option<String>,
    pub created_by_moderator_id: i64,
    pub created_at: DateTime<Utc>,
}
