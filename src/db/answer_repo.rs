use sqlx::PgPool;

use crate::domain::JoinRequestAnswer;
use crate::error::AppError;

pub struct AnswerRepo;

impl AnswerRepo {
    pub async fn create(
        pool: &PgPool,
        join_request_id: i64,
        community_question_id: i64,
        answer_text: &str,
    ) -> Result<JoinRequestAnswer, AppError> {
        let answer = sqlx::query_as!(
            JoinRequestAnswer,
            "INSERT INTO join_request_answers (join_request_id, community_question_id, answer_text)
             VALUES ($1, $2, $3)
             RETURNING id, join_request_id, community_question_id, answer_text, created_at",
            join_request_id,
            community_question_id,
            answer_text,
        )
        .fetch_one(pool)
        .await?;

        Ok(answer)
    }

    pub async fn find_by_join_request_id(
        pool: &PgPool,
        join_request_id: i64,
    ) -> Result<Vec<JoinRequestAnswer>, AppError> {
        let answers = sqlx::query_as!(
            JoinRequestAnswer,
            "SELECT id, join_request_id, community_question_id, answer_text, created_at
             FROM join_request_answers
             WHERE join_request_id = $1
             ORDER BY id ASC",
            join_request_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(answers)
    }
}
