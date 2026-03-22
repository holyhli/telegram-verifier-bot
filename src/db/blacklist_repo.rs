use sqlx::PgPool;

use crate::domain::{BlacklistEntry, ScopeType};
use crate::error::AppError;

pub struct BlacklistRepo;

impl BlacklistRepo {
    pub async fn find_by_telegram_user_id(
        pool: &PgPool,
        telegram_user_id: i64,
    ) -> Result<Vec<BlacklistEntry>, AppError> {
        let entries = sqlx::query_as!(
            BlacklistEntry,
            r#"SELECT id, telegram_user_id,
                 scope_type as "scope_type: ScopeType",
                 community_id, reason, created_by_moderator_id, created_at
             FROM blacklist_entries
             WHERE telegram_user_id = $1"#,
            telegram_user_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(entries)
    }

    pub async fn create(
        pool: &PgPool,
        telegram_user_id: i64,
        scope_type: ScopeType,
        community_id: Option<i64>,
        reason: Option<&str>,
        created_by_moderator_id: i64,
    ) -> Result<BlacklistEntry, AppError> {
        let entry = sqlx::query_as!(
            BlacklistEntry,
            r#"INSERT INTO blacklist_entries (telegram_user_id, scope_type, community_id, reason, created_by_moderator_id)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, telegram_user_id,
                 scope_type as "scope_type: ScopeType",
                 community_id, reason, created_by_moderator_id, created_at"#,
            telegram_user_id,
            scope_type as ScopeType,
            community_id,
            reason,
            created_by_moderator_id,
        )
        .fetch_one(pool)
        .await?;

        Ok(entry)
    }
}
