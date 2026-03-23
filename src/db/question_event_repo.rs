use sqlx::PgPool;

use crate::domain::{QuestionEvent, QuestionEventType};
use crate::error::AppError;

pub struct QuestionEventRepo;

impl QuestionEventRepo {
    pub async fn create(
        pool: &PgPool,
        join_request_id: i64,
        community_question_id: i64,
        applicant_id: i64,
        event_type: QuestionEventType,
        metadata: Option<serde_json::Value>,
    ) -> Result<QuestionEvent, AppError> {
        let event = sqlx::query_as!(
            QuestionEvent,
            r#"INSERT INTO question_events (join_request_id, community_question_id, applicant_id, event_type, metadata)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, join_request_id, community_question_id, applicant_id,
                       event_type as "event_type: QuestionEventType", metadata, created_at"#,
            join_request_id,
            community_question_id,
            applicant_id,
            event_type as QuestionEventType,
            metadata,
        )
        .fetch_one(pool)
        .await?;

        Ok(event)
    }

    pub async fn find_by_join_request_id(
        pool: &PgPool,
        join_request_id: i64,
    ) -> Result<Vec<QuestionEvent>, AppError> {
        let events = sqlx::query_as!(
            QuestionEvent,
            r#"SELECT id, join_request_id, community_question_id, applicant_id,
                    event_type as "event_type: QuestionEventType", metadata, created_at
             FROM question_events
             WHERE join_request_id = $1
             ORDER BY created_at ASC"#,
            join_request_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(events)
    }

    pub async fn count_validation_failures(
        pool: &PgPool,
        join_request_id: i64,
    ) -> Result<Vec<(i64, i64)>, AppError> {
        let rows = sqlx::query_as::<_, (i64, i64)>(
            "SELECT community_question_id, COUNT(*) as count
             FROM question_events
             WHERE join_request_id = $1 AND event_type = 'validation_failed'
             GROUP BY community_question_id",
        )
        .bind(join_request_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
