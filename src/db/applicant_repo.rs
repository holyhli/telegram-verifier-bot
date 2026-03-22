use sqlx::PgPool;

use crate::domain::Applicant;
use crate::error::AppError;

pub struct ApplicantRepo;

impl ApplicantRepo {
    pub async fn find_or_create_by_telegram_user_id(
        pool: &PgPool,
        telegram_user_id: i64,
        first_name: &str,
        last_name: Option<&str>,
        username: Option<&str>,
    ) -> Result<Applicant, AppError> {
        let applicant = sqlx::query_as!(
            Applicant,
            "INSERT INTO applicants (telegram_user_id, first_name, last_name, username)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (telegram_user_id) DO UPDATE
                SET first_name = EXCLUDED.first_name,
                    last_name = EXCLUDED.last_name,
                    username = EXCLUDED.username,
                    updated_at = NOW()
             RETURNING id, telegram_user_id, first_name, last_name, username, created_at, updated_at",
            telegram_user_id,
            first_name,
            last_name,
            username,
        )
        .fetch_one(pool)
        .await?;

        Ok(applicant)
    }

    pub async fn update_profile(
        pool: &PgPool,
        id: i64,
        first_name: &str,
        last_name: Option<&str>,
        username: Option<&str>,
    ) -> Result<Applicant, AppError> {
        let applicant = sqlx::query_as!(
            Applicant,
            "UPDATE applicants
             SET first_name = $2, last_name = $3, username = $4, updated_at = NOW()
             WHERE id = $1
             RETURNING id, telegram_user_id, first_name, last_name, username, created_at, updated_at",
            id,
            first_name,
            last_name,
            username,
        )
        .fetch_optional(pool)
        .await?;

        applicant.ok_or_else(|| AppError::NotFound(format!("applicant {id}")))
    }
}
