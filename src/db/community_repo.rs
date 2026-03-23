use sqlx::PgPool;

use crate::domain::{Community, CommunityQuestion};
use crate::error::AppError;

pub struct CommunityRepo;

impl CommunityRepo {
    pub async fn find_by_id(
        pool: &PgPool,
        id: i64,
    ) -> Result<Option<Community>, AppError> {
        let community = sqlx::query_as!(
            Community,
            "SELECT id, telegram_chat_id, title, slug, is_active, created_at, updated_at
             FROM communities WHERE id = $1",
            id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(community)
    }

    pub async fn find_by_telegram_chat_id(
        pool: &PgPool,
        telegram_chat_id: i64,
    ) -> Result<Option<Community>, AppError> {
        let community = sqlx::query_as!(
            Community,
            "SELECT id, telegram_chat_id, title, slug, is_active, created_at, updated_at
             FROM communities WHERE telegram_chat_id = $1",
            telegram_chat_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(community)
    }

    pub async fn find_active_questions(
        pool: &PgPool,
        community_id: i64,
    ) -> Result<Vec<CommunityQuestion>, AppError> {
        let questions = sqlx::query_as!(
            CommunityQuestion,
            "SELECT id, community_id, question_key, question_text, question_text_uk, required, position, is_active, created_at, updated_at
             FROM community_questions
             WHERE community_id = $1 AND is_active = TRUE
             ORDER BY position ASC",
            community_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(questions)
    }

    pub async fn find_all(pool: &PgPool) -> Result<Vec<Community>, AppError> {
        let communities = sqlx::query_as!(
            Community,
            "SELECT id, telegram_chat_id, title, slug, is_active, created_at, updated_at
             FROM communities
             WHERE is_active = TRUE
             ORDER BY title ASC"
        )
        .fetch_all(pool)
        .await?;

        Ok(communities)
    }
}
