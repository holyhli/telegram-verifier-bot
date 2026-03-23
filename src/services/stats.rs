use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::db::QuestionEventRepo;
use crate::domain::{QuestionEvent, QuestionEventType};
use crate::error::AppError;



#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveApplicantInfo {
    pub join_request_id: i64,
    pub applicant_id: i64,
    pub name: Option<String>,
    pub username: Option<String>,
    pub current_question_position: i32,
    pub total_questions: i32,
    pub current_question_text: String,
    pub time_on_current_secs: i64,
    pub time_started_secs: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QuestionTiming {
    pub community_question_id: i64,
    pub position: i32,
    pub question_text: String,
    pub duration_secs: Option<i64>,
    pub retry_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplicantSummary {
    pub join_request_id: i64,
    pub applicant_id: i64,
    pub name: Option<String>,
    pub username: Option<String>,
    pub status: String,
    pub question_timings: Vec<QuestionTiming>,
    pub total_time_secs: Option<i64>,
    pub total_retries: i64,
}



#[derive(Debug, sqlx::FromRow)]
struct ActiveApplicantRow {
    join_request_id: i64,
    applicant_id: i64,
    name: String,
    username: Option<String>,
    current_question_position: i32,
    total_questions: i32,
    current_question_text: String,
    time_on_current_secs: i64,
    time_started_secs: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct PeriodJoinRequestRow {
    join_request_id: i64,
    applicant_id: i64,
    name: String,
    username: Option<String>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct QuestionInfoRow {
    id: i64,
    position: i32,
    question_text: String,
}



pub struct StatsService;

impl StatsService {
    pub async fn get_active_applicants(
        pool: &PgPool,
        community_id: i64,
    ) -> Result<Vec<ActiveApplicantInfo>, AppError> {
        let rows = sqlx::query_as::<_, ActiveApplicantRow>(
            r#"SELECT
                jr.id AS join_request_id,
                jr.applicant_id,
                a.first_name AS name,
                a.username,
                s.current_question_position,
                (SELECT COUNT(*)::INT4 FROM community_questions
                 WHERE community_id = jr.community_id AND is_active = TRUE) AS total_questions,
                q.question_text AS current_question_text,
                COALESCE(
                    EXTRACT(EPOCH FROM (NOW() - (
                        SELECT MAX(qe.created_at) FROM question_events qe
                        WHERE qe.join_request_id = jr.id
                          AND qe.event_type = 'question_presented'
                    )))::BIGINT,
                    0
                ) AS time_on_current_secs,
                EXTRACT(EPOCH FROM (NOW() - jr.created_at))::BIGINT AS time_started_secs
            FROM join_requests jr
            INNER JOIN applicants a ON a.id = jr.applicant_id
            INNER JOIN applicant_sessions s
                ON s.join_request_id = jr.id
               AND s.state = 'awaiting_answer'
            INNER JOIN community_questions q
                ON q.community_id = jr.community_id
               AND q.position = s.current_question_position
               AND q.is_active = TRUE
            WHERE jr.community_id = $1
              AND jr.status = 'questionnaire_in_progress'
            ORDER BY jr.created_at ASC"#,
        )
        .bind(community_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ActiveApplicantInfo {
                join_request_id: r.join_request_id,
                applicant_id: r.applicant_id,
                name: Some(r.name),
                username: r.username,
                current_question_position: r.current_question_position,
                total_questions: r.total_questions,
                current_question_text: r.current_question_text,
                time_on_current_secs: r.time_on_current_secs,
                time_started_secs: r.time_started_secs,
            })
            .collect())
    }

    pub async fn get_period_summary(
        pool: &PgPool,
        community_id: i64,
        period_start: DateTime<Utc>,
    ) -> Result<Vec<ApplicantSummary>, AppError> {
        let questions = sqlx::query_as::<_, QuestionInfoRow>(
            "SELECT id, position, question_text FROM community_questions \
             WHERE community_id = $1 AND is_active = TRUE ORDER BY position",
        )
        .bind(community_id)
        .fetch_all(pool)
        .await?;

        let question_map: std::collections::HashMap<i64, (i32, String)> = questions
            .into_iter()
            .map(|q| (q.id, (q.position, q.question_text)))
            .collect();

        let rows = sqlx::query_as::<_, PeriodJoinRequestRow>(
            r#"SELECT
                jr.id AS join_request_id,
                jr.applicant_id,
                a.first_name AS name,
                a.username,
                jr.status AS status
            FROM join_requests jr
            INNER JOIN applicants a ON a.id = jr.applicant_id
            WHERE jr.community_id = $1
              AND jr.created_at >= $2
            ORDER BY jr.created_at ASC"#,
        )
        .bind(community_id)
        .bind(period_start)
        .fetch_all(pool)
        .await?;

        let mut summaries = Vec::with_capacity(rows.len());

        for row in rows {
            let events =
                QuestionEventRepo::find_by_join_request_id(pool, row.join_request_id).await?;
            let mut timings = Self::compute_per_question_timing(&events);

            for timing in &mut timings {
                if let Some((pos, text)) = question_map.get(&timing.community_question_id) {
                    timing.position = *pos;
                    timing.question_text = text.clone();
                }
            }

            let total_time: i64 = timings.iter().filter_map(|t| t.duration_secs).sum();
            let total_retries: i64 = timings.iter().map(|t| t.retry_count).sum();

            summaries.push(ApplicantSummary {
                join_request_id: row.join_request_id,
                applicant_id: row.applicant_id,
                name: Some(row.name),
                username: row.username,
                status: row.status,
                question_timings: timings,
                total_time_secs: if total_time > 0 {
                    Some(total_time)
                } else {
                    None
                },
                total_retries,
            });
        }

        Ok(summaries)
    }

    /// `position` and `question_text` are left as defaults — callers enrich from community_questions.
    pub fn compute_per_question_timing(events: &[QuestionEvent]) -> Vec<QuestionTiming> {
        use std::collections::BTreeMap;

        let mut groups: BTreeMap<i64, Vec<&QuestionEvent>> = BTreeMap::new();
        for event in events {
            groups
                .entry(event.community_question_id)
                .or_default()
                .push(event);
        }

        groups
            .into_iter()
            .map(|(community_question_id, evts)| {
                let presented = evts
                    .iter()
                    .find(|e| e.event_type == QuestionEventType::QuestionPresented);
                let accepted = evts
                    .iter()
                    .find(|e| e.event_type == QuestionEventType::AnswerAccepted);

                let duration_secs = match (presented, accepted) {
                    (Some(p), Some(a)) => Some((a.created_at - p.created_at).num_seconds()),
                    _ => None,
                };

                let retry_count = evts
                    .iter()
                    .filter(|e| e.event_type == QuestionEventType::ValidationFailed)
                    .count() as i64;

                QuestionTiming {
                    community_question_id,
                    position: 0,
                    question_text: String::new(),
                    duration_secs,
                    retry_count,
                }
            })
            .collect()
    }
}
