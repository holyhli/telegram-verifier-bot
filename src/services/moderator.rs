use chrono::{DateTime, Utc};
use sqlx::PgPool;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::bot::handlers::TelegramApi;
use crate::db::JoinRequestRepo;
use crate::domain::{JoinRequest, JoinRequestStatus};
use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ModeratorCardContext {
    pub join_request_id: i64,
    pub community_title: String,
    pub community_chat_id: i64,
    pub applicant_first_name: String,
    pub applicant_last_name: Option<String>,
    pub applicant_username: Option<String>,
    pub applicant_telegram_user_id: i64,
    pub applicant_chat_id: i64,
    pub join_request_date: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ModeratorCardAnswer {
    pub position: i32,
    pub question_text: String,
    pub answer_text: String,
}

pub fn render_moderator_card(
    context: &ModeratorCardContext,
    answers: &[ModeratorCardAnswer],
) -> String {
    let applicant_name = format!(
        "{} {}",
        context.applicant_first_name,
        context.applicant_last_name.clone().unwrap_or_default()
    )
    .trim()
    .to_string();

    let username = context
        .applicant_username
        .as_ref()
        .map(|value| format!("@{value}"))
        .unwrap_or_else(|| "not set".to_string());

    let mut text = format!(
        "<b>📋 New Join Request</b>\n\
<b>Community:</b> {}\n\
<b>Applicant:</b> {}\n\
<b>Username:</b> {}\n\
<b>Telegram ID:</b> <code>{}</code>\n\
<b>Requested at:</b> {} UTC\n\
<b>Completed at:</b> {} UTC\n\n\
<b>📝 Answers</b>",
        escape_html(&context.community_title),
        escape_html(&applicant_name),
        escape_html(&username),
        context.applicant_telegram_user_id,
        format_utc(context.join_request_date),
        format_utc(context.completed_at),
    );

    for answer in answers {
        text.push_str(&format!(
            "\n{}. <b>{}:</b> {}",
            answer.position,
            escape_html(&answer.question_text),
            escape_html(&answer.answer_text),
        ));
    }

    text.push_str(&format!(
        "\n\n<b>Status:</b> Submitted\n<b>Request ID:</b> <code>{}</code>",
        context.join_request_id
    ));

    text
}

pub async fn send_moderator_card(
    api: &dyn TelegramApi,
    pool: &PgPool,
    join_request_id: i64,
    moderator_chat_id: i64,
) -> Result<JoinRequest, AppError> {
    let context = load_moderator_card_context(pool, join_request_id).await?;
    let answers = load_moderator_card_answers(pool, join_request_id).await?;
    let text = render_moderator_card(&context, &answers);

    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("✅ Approve", format!("a:{join_request_id}")),
        InlineKeyboardButton::callback("❌ Reject", format!("r:{join_request_id}")),
        InlineKeyboardButton::callback("🚫 Ban", format!("b:{join_request_id}")),
    ]]);

    let sent_message_id = api
        .send_message_html(moderator_chat_id, text, Some(keyboard))
        .await
        .map_err(|err| AppError::Telegram(err.to_string()))?;

    let updated = sqlx::query_as::<_, JoinRequest>(
        r#"UPDATE join_requests
           SET submitted_to_moderators_at = NOW(),
               moderator_message_chat_id = $2,
               moderator_message_id = $3,
               updated_at = NOW()
           WHERE id = $1
           RETURNING id, community_id, applicant_id, telegram_user_chat_id,
               status, telegram_join_request_date, submitted_to_moderators_at, approved_at,
               rejected_at, moderator_message_chat_id, moderator_message_id,
               created_at, updated_at, reminder_sent_at"#,
    )
    .bind(join_request_id)
    .bind(moderator_chat_id)
    .bind(sent_message_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("join request {join_request_id}")))?;

    Ok(updated)
}

pub async fn load_moderator_card_context(
    pool: &PgPool,
    join_request_id: i64,
) -> Result<ModeratorCardContext, AppError> {
    let context = sqlx::query_as::<_, ModeratorCardContext>(
        r#"SELECT jr.id AS join_request_id,
                  c.title AS community_title,
                  c.telegram_chat_id AS community_chat_id,
                  a.first_name AS applicant_first_name,
                  a.last_name AS applicant_last_name,
                  a.username AS applicant_username,
                  a.telegram_user_id AS applicant_telegram_user_id,
                  jr.telegram_user_chat_id AS applicant_chat_id,
                  jr.telegram_join_request_date AS join_request_date,
                  jr.updated_at AS completed_at
           FROM join_requests jr
           INNER JOIN communities c ON c.id = jr.community_id
           INNER JOIN applicants a ON a.id = jr.applicant_id
           WHERE jr.id = $1"#,
    )
    .bind(join_request_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("join request {join_request_id}")))?;

    Ok(context)
}

pub async fn load_moderator_card_answers(
    pool: &PgPool,
    join_request_id: i64,
) -> Result<Vec<ModeratorCardAnswer>, AppError> {
    let rows = sqlx::query_as::<_, ModeratorCardAnswer>(
        r#"SELECT cq.position,
                  cq.question_text,
                  jra.answer_text
           FROM join_request_answers jra
           INNER JOIN community_questions cq ON cq.id = jra.community_question_id
           WHERE jra.join_request_id = $1
           ORDER BY cq.position ASC"#,
    )
    .bind(join_request_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn load_submitted_join_request(
    pool: &PgPool,
    join_request_id: i64,
) -> Result<JoinRequest, AppError> {
    let join_request = JoinRequestRepo::find_by_id(pool, join_request_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("join request {join_request_id}")))?;

    if join_request.status != JoinRequestStatus::Submitted {
        return Err(AppError::AlreadyProcessed {
            join_request_id,
            current_status: join_request.status,
        });
    }

    Ok(join_request)
}

fn format_utc(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
