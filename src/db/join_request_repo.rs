use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::domain::{JoinRequest, JoinRequestStatus};
use crate::error::AppError;

pub struct JoinRequestRepo;

impl JoinRequestRepo {
    pub async fn create(
        pool: &PgPool,
        community_id: i64,
        applicant_id: i64,
        telegram_user_chat_id: i64,
        telegram_join_request_date: DateTime<Utc>,
    ) -> Result<JoinRequest, AppError> {
        let jr = sqlx::query_as!(
            JoinRequest,
            r#"INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, telegram_join_request_date)
             VALUES ($1, $2, $3, $4)
             RETURNING id, community_id, applicant_id, telegram_user_chat_id,
                 status as "status: JoinRequestStatus",
                 telegram_join_request_date, submitted_to_moderators_at, approved_at, rejected_at,
                 moderator_message_chat_id, moderator_message_id,
                 created_at, updated_at, reminder_sent_at"#,
            community_id,
            applicant_id,
            telegram_user_chat_id,
            telegram_join_request_date,
        )
        .fetch_one(pool)
        .await?;

        Ok(jr)
    }

    pub async fn find_by_id(
        pool: &PgPool,
        id: i64,
    ) -> Result<Option<JoinRequest>, AppError> {
        let jr = sqlx::query_as!(
            JoinRequest,
            r#"SELECT id, community_id, applicant_id, telegram_user_chat_id,
                 status as "status: JoinRequestStatus",
                 telegram_join_request_date, submitted_to_moderators_at, approved_at, rejected_at,
                 moderator_message_chat_id, moderator_message_id,
                 created_at, updated_at, reminder_sent_at
             FROM join_requests WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(jr)
    }

    pub async fn find_active_by_telegram_user_id_and_chat_id(
        pool: &PgPool,
        telegram_user_id: i64,
        user_chat_id: i64,
    ) -> Result<Option<JoinRequest>, AppError> {
        let jr = sqlx::query_as!(
            JoinRequest,
            r#"SELECT jr.id, jr.community_id, jr.applicant_id, jr.telegram_user_chat_id,
                 jr.status as "status: JoinRequestStatus",
                 jr.telegram_join_request_date, jr.submitted_to_moderators_at, jr.approved_at, jr.rejected_at,
                 jr.moderator_message_chat_id, jr.moderator_message_id,
                 jr.created_at, jr.updated_at, jr.reminder_sent_at
             FROM join_requests jr
             JOIN applicants a ON jr.applicant_id = a.id
             WHERE a.telegram_user_id = $1 AND jr.telegram_user_chat_id = $2
               AND jr.status NOT IN ('approved', 'rejected', 'banned', 'expired', 'cancelled')
             ORDER BY jr.created_at DESC
             LIMIT 1"#,
            telegram_user_id,
            user_chat_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(jr)
    }

    pub async fn find_active_for_applicant_in_community(
        pool: &PgPool,
        applicant_id: i64,
        community_id: i64,
    ) -> Result<Option<JoinRequest>, AppError> {
        let jr = sqlx::query_as!(
            JoinRequest,
            r#"SELECT id, community_id, applicant_id, telegram_user_chat_id,
                 status as "status: JoinRequestStatus",
                 telegram_join_request_date, submitted_to_moderators_at, approved_at, rejected_at,
                 moderator_message_chat_id, moderator_message_id,
                 created_at, updated_at, reminder_sent_at
             FROM join_requests
             WHERE applicant_id = $1 AND community_id = $2
               AND status NOT IN ('approved', 'rejected', 'banned', 'expired', 'cancelled')"#,
            applicant_id,
            community_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(jr)
    }

    /// Updates join request status with optimistic locking.
    /// Checks `id`, `status`, and `updated_at` in WHERE clause to detect concurrent modifications.
    /// Returns `AlreadyProcessed` if another process modified the row first.
    pub async fn update_status(
        pool: &PgPool,
        id: i64,
        from_status: JoinRequestStatus,
        to_status: JoinRequestStatus,
        expected_updated_at: DateTime<Utc>,
    ) -> Result<JoinRequest, AppError> {
        if !from_status.can_transition_to(&to_status) {
            return Err(AppError::InvalidStateTransition {
                from: from_status,
                to: to_status,
            });
        }

        let row = sqlx::query_as!(
            JoinRequest,
            r#"UPDATE join_requests
             SET status = $2, updated_at = NOW()
             WHERE id = $1 AND status = $3 AND updated_at = $4
             RETURNING id, community_id, applicant_id, telegram_user_chat_id,
                 status as "status: JoinRequestStatus",
                 telegram_join_request_date, submitted_to_moderators_at, approved_at, rejected_at,
                 moderator_message_chat_id, moderator_message_id,
                 created_at, updated_at, reminder_sent_at"#,
            id,
            to_status as JoinRequestStatus,
            from_status as JoinRequestStatus,
            expected_updated_at,
        )
        .fetch_optional(pool)
        .await?;

        row.ok_or_else(|| AppError::AlreadyProcessed {
            join_request_id: id,
            current_status: from_status,
        })
    }

    pub async fn find_expired(
        pool: &PgPool,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<JoinRequest>, AppError> {
        let rows = sqlx::query_as::<_, JoinRequest>(
            r#"SELECT id, community_id, applicant_id, telegram_user_chat_id,
                 status, telegram_join_request_date, submitted_to_moderators_at,
                 approved_at, rejected_at, moderator_message_chat_id, moderator_message_id,
                 created_at, updated_at, reminder_sent_at
             FROM join_requests
             WHERE status IN ('pending_contact', 'questionnaire_in_progress')
               AND created_at < $1"#,
        )
        .bind(cutoff)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn update_reminder_sent_at(
        pool: &PgPool,
        id: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE join_requests SET reminder_sent_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn find_needing_reminder(
        pool: &PgPool,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<JoinRequest>, AppError> {
        let rows = sqlx::query_as!(
            JoinRequest,
            r#"SELECT id, community_id, applicant_id, telegram_user_chat_id,
                 status as "status: JoinRequestStatus",
                 telegram_join_request_date, submitted_to_moderators_at, approved_at, rejected_at,
                 moderator_message_chat_id, moderator_message_id,
                 created_at, updated_at, reminder_sent_at
             FROM join_requests
             WHERE status = 'questionnaire_in_progress'
               AND created_at < $1
               AND reminder_sent_at IS NULL"#,
            cutoff,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
