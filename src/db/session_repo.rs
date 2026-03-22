use sqlx::PgPool;

#[allow(unused_imports)]
use crate::domain::SessionState;
use crate::domain::ApplicantSession;
use crate::error::AppError;

pub struct SessionRepo;

impl SessionRepo {
    pub async fn create(
        pool: &PgPool,
        join_request_id: i64,
        current_question_position: i32,
    ) -> Result<ApplicantSession, AppError> {
        let session = sqlx::query_as!(
            ApplicantSession,
            r#"INSERT INTO applicant_sessions (join_request_id, current_question_position)
             VALUES ($1, $2)
             RETURNING id, join_request_id, current_question_position,
                 state as "state: SessionState",
                 created_at, updated_at"#,
            join_request_id,
            current_question_position,
        )
        .fetch_one(pool)
        .await?;

        Ok(session)
    }

    pub async fn find_active_by_join_request_id(
        pool: &PgPool,
        join_request_id: i64,
    ) -> Result<Option<ApplicantSession>, AppError> {
        let session = sqlx::query_as!(
            ApplicantSession,
            r#"SELECT id, join_request_id, current_question_position,
                 state as "state: SessionState",
                 created_at, updated_at
             FROM applicant_sessions
             WHERE join_request_id = $1 AND state = 'awaiting_answer'"#,
            join_request_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(session)
    }

    pub async fn advance_question(
        pool: &PgPool,
        id: i64,
        new_position: i32,
    ) -> Result<ApplicantSession, AppError> {
        let session = sqlx::query_as!(
            ApplicantSession,
            r#"UPDATE applicant_sessions
             SET current_question_position = $2, updated_at = NOW()
             WHERE id = $1 AND state = 'awaiting_answer'
             RETURNING id, join_request_id, current_question_position,
                 state as "state: SessionState",
                 created_at, updated_at"#,
            id,
            new_position,
        )
        .fetch_optional(pool)
        .await?;

        session.ok_or_else(|| AppError::NotFound(format!("active session {id}")))
    }

    pub async fn complete(
        pool: &PgPool,
        id: i64,
    ) -> Result<ApplicantSession, AppError> {
        let session = sqlx::query_as!(
            ApplicantSession,
            r#"UPDATE applicant_sessions
             SET state = 'completed', updated_at = NOW()
             WHERE id = $1 AND state = 'awaiting_answer'
             RETURNING id, join_request_id, current_question_position,
                 state as "state: SessionState",
                 created_at, updated_at"#,
            id,
        )
        .fetch_optional(pool)
        .await?;

        session.ok_or_else(|| AppError::NotFound(format!("active session {id}")))
    }

    pub async fn expire(
        pool: &PgPool,
        id: i64,
    ) -> Result<ApplicantSession, AppError> {
        let session = sqlx::query_as!(
            ApplicantSession,
            r#"UPDATE applicant_sessions
             SET state = 'expired', updated_at = NOW()
             WHERE id = $1 AND state = 'awaiting_answer'
             RETURNING id, join_request_id, current_question_position,
                 state as "state: SessionState",
                 created_at, updated_at"#,
            id,
        )
        .fetch_optional(pool)
        .await?;

        session.ok_or_else(|| AppError::NotFound(format!("active session {id}")))
    }
}
