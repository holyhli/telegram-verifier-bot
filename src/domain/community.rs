use chrono::{DateTime, Utc};

/// A Telegram community (group/supergroup) managed by the bot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Community {
    pub id: i64,
    pub telegram_chat_id: i64,
    pub title: String,
    pub slug: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A verification question configured for a community.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct CommunityQuestion {
    pub id: i64,
    pub community_id: i64,
    pub question_key: String,
    pub question_text: String,
    pub required: bool,
    pub position: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
