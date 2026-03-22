use sqlx::PgPool;

use crate::domain::{ActionType, ModerationAction};
use crate::error::AppError;

pub struct ModerationActionRepo;

impl ModerationActionRepo {
    pub async fn create(
        pool: &PgPool,
        join_request_id: i64,
        moderator_telegram_user_id: i64,
        action_type: ActionType,
    ) -> Result<ModerationAction, AppError> {
        let action = sqlx::query_as!(
            ModerationAction,
            r#"INSERT INTO moderation_actions (join_request_id, moderator_telegram_user_id, action_type)
             VALUES ($1, $2, $3)
             RETURNING id, join_request_id, moderator_telegram_user_id,
                 action_type as "action_type: ActionType",
                 created_at"#,
            join_request_id,
            moderator_telegram_user_id,
            action_type as ActionType,
        )
        .fetch_one(pool)
        .await?;

        Ok(action)
    }

    pub async fn find_by_join_request_id(
        pool: &PgPool,
        join_request_id: i64,
    ) -> Result<Vec<ModerationAction>, AppError> {
        let actions = sqlx::query_as!(
            ModerationAction,
            r#"SELECT id, join_request_id, moderator_telegram_user_id,
                 action_type as "action_type: ActionType",
                 created_at
             FROM moderation_actions
             WHERE join_request_id = $1
             ORDER BY created_at ASC"#,
            join_request_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(actions)
    }
}
