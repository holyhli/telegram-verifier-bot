use async_trait::async_trait;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use teloxide::types::InlineKeyboardMarkup;
use teloxide::RequestError;

use verifier_bot::bot::handlers::TelegramApi;
use verifier_bot::config::BotSettings;
use verifier_bot::services::expiry::process_tick;

#[derive(Debug, Clone)]
struct FakeTelegramApi {
    sent_messages: Arc<Mutex<Vec<(i64, String)>>>,
    declined_requests: Arc<Mutex<Vec<(i64, i64)>>>,
    edited_messages_with_markup: Arc<Mutex<Vec<(i64, i32, String, Option<Vec<Vec<(String, String)>>>)>>>,
}

impl FakeTelegramApi {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            declined_requests: Arc::new(Mutex::new(Vec::new())),
            edited_messages_with_markup: Arc::new(Mutex::new(vec![])),
        }
    }

    fn sent_messages(&self) -> Vec<(i64, String)> {
        self.sent_messages.lock().unwrap().clone()
    }

    fn declined_requests(&self) -> Vec<(i64, i64)> {
        self.declined_requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl TelegramApi for FakeTelegramApi {
    async fn send_message(&self, chat_id: i64, text: String) -> Result<(), RequestError> {
        self.sent_messages.lock().unwrap().push((chat_id, text));
        Ok(())
    }

    async fn send_message_html(
        &self,
        _chat_id: i64,
        _text: String,
        _reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<i64, RequestError> {
        Ok(1)
    }

    async fn edit_message_html(
        &self,
        _chat_id: i64,
        _message_id: i64,
        _text: String,
    ) -> Result<(), RequestError> {
        Ok(())
    }

    async fn clear_message_reply_markup(
        &self,
        _chat_id: i64,
        _message_id: i64,
    ) -> Result<(), RequestError> {
        Ok(())
    }

    async fn answer_callback_query(
        &self,
        _callback_query_id: String,
        _text: String,
    ) -> Result<(), RequestError> {
        Ok(())
    }

    async fn approve_chat_join_request(
        &self,
        _chat_id: i64,
        _user_id: i64,
    ) -> Result<(), RequestError> {
        Ok(())
    }

    async fn decline_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError> {
        self.declined_requests.lock().unwrap().push((chat_id, user_id));
        Ok(())
    }

    async fn edit_message_html_with_markup(
        &self,
        chat_id: i64,
        message_id: i32,
        text: String,
        reply_markup: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<(), RequestError> {
        self.edited_messages_with_markup
            .lock()
            .unwrap()
            .push((chat_id, message_id, text, reply_markup));
        Ok(())
    }

    async fn send_message_with_inline_keyboard(
        &self,
        _chat_id: i64,
        _text: String,
        _keyboard: Vec<Vec<(String, String)>>,
    ) -> Result<(), RequestError> {
        Ok(())
    }
}

fn default_settings() -> BotSettings {
    BotSettings {
        application_timeout_minutes: 60,
        reminder_before_expiry_minutes: 15,
    }
}

async fn seed_community(pool: &PgPool, chat_id: i64, slug: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO communities (telegram_chat_id, title, slug) VALUES ($1, 'Test Community', $2) RETURNING id",
    )
    .bind(chat_id)
    .bind(slug)
    .fetch_one(pool)
    .await
    .expect("seed community");
    id
}

async fn seed_applicant(pool: &PgPool, telegram_user_id: i64) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO applicants (telegram_user_id, first_name) VALUES ($1, 'TestUser') RETURNING id",
    )
    .bind(telegram_user_id)
    .fetch_one(pool)
    .await
    .expect("seed applicant");
    id
}

async fn seed_join_request(
    pool: &PgPool,
    community_id: i64,
    applicant_id: i64,
    user_chat_id: i64,
    status: &str,
    created_minutes_ago: i64,
) -> i64 {
    let created_at = Utc::now() - Duration::minutes(created_minutes_ago);
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO join_requests \
         (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $5, $5) RETURNING id",
    )
    .bind(community_id)
    .bind(applicant_id)
    .bind(user_chat_id)
    .bind(status)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .expect("seed join request");
    id
}

async fn seed_session(pool: &PgPool, join_request_id: i64) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO applicant_sessions (join_request_id, current_question_position) \
         VALUES ($1, 1) RETURNING id",
    )
    .bind(join_request_id)
    .fetch_one(pool)
    .await
    .expect("seed session");
    id
}

#[sqlx::test(migrations = "./migrations")]
async fn expiry_processes_pending_contact_request(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1001000000001, "expiry-test-1").await;
    let applicant_id = seed_applicant(&pool, 10001).await;
    let jr_id = seed_join_request(
        &pool, community_id, applicant_id, 10001, "pending_contact", 90,
    )
    .await;

    let api = FakeTelegramApi::new();
    process_tick(&api, &pool, &default_settings())
        .await
        .expect("process_tick");

    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM join_requests WHERE id = $1")
            .bind(jr_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "expired");

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].1.contains("timed out"));

    let declined = api.declined_requests();
    assert_eq!(declined.len(), 1);
    assert_eq!(declined[0], (-1001000000001, 10001));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn expiry_questionnaire_request_also_expires_session(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1001000000002, "expiry-test-2").await;
    let applicant_id = seed_applicant(&pool, 10002).await;
    let jr_id = seed_join_request(
        &pool,
        community_id,
        applicant_id,
        10002,
        "questionnaire_in_progress",
        90,
    )
    .await;
    let session_id = seed_session(&pool, jr_id).await;

    let api = FakeTelegramApi::new();
    process_tick(&api, &pool, &default_settings())
        .await
        .expect("process_tick");

    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM join_requests WHERE id = $1")
            .bind(jr_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "expired");

    let (session_state,): (String,) =
        sqlx::query_as("SELECT state::text FROM applicant_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(session_state, "expired");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn expiry_skips_submitted_requests(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1001000000003, "expiry-test-3").await;
    let applicant_id = seed_applicant(&pool, 10003).await;
    let jr_id =
        seed_join_request(&pool, community_id, applicant_id, 10003, "submitted", 90).await;

    let api = FakeTelegramApi::new();
    process_tick(&api, &pool, &default_settings())
        .await
        .expect("process_tick");

    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM join_requests WHERE id = $1")
            .bind(jr_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "submitted");

    assert!(api.sent_messages().is_empty());
    assert!(api.declined_requests().is_empty());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn expiry_reminder_sent_for_eligible_request(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1001000000004, "expiry-test-4").await;
    let applicant_id = seed_applicant(&pool, 10004).await;
    let jr_id = seed_join_request(
        &pool,
        community_id,
        applicant_id,
        10004,
        "questionnaire_in_progress",
        50,
    )
    .await;

    let api = FakeTelegramApi::new();
    process_tick(&api, &pool, &default_settings())
        .await
        .expect("process_tick");

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].1.contains("reminder"));
    assert!(sent[0].1.contains("Test Community"));

    let (reminder_set,): (bool,) =
        sqlx::query_as("SELECT reminder_sent_at IS NOT NULL FROM join_requests WHERE id = $1")
            .bind(jr_id)
            .fetch_one(&pool)
            .await?;
    assert!(reminder_set);

    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM join_requests WHERE id = $1")
            .bind(jr_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "questionnaire_in_progress");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn expiry_no_duplicate_reminders_sent(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1001000000005, "expiry-test-5").await;
    let applicant_id = seed_applicant(&pool, 10005).await;
    let jr_id = seed_join_request(
        &pool,
        community_id,
        applicant_id,
        10005,
        "questionnaire_in_progress",
        50,
    )
    .await;

    sqlx::query("UPDATE join_requests SET reminder_sent_at = NOW() WHERE id = $1")
        .bind(jr_id)
        .execute(&pool)
        .await?;

    let api = FakeTelegramApi::new();
    process_tick(&api, &pool, &default_settings())
        .await
        .expect("process_tick");

    assert!(api.sent_messages().is_empty());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn expiry_blacklist_auto_decline_global_and_community(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1001000000006, "expiry-test-6").await;

    sqlx::query(
        "INSERT INTO blacklist_entries \
         (telegram_user_id, scope_type, community_id, reason, created_by_moderator_id) \
         VALUES ($1, 'global', NULL, 'spam', 999)",
    )
    .bind(10006_i64)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO blacklist_entries \
         (telegram_user_id, scope_type, community_id, reason, created_by_moderator_id) \
         VALUES ($1, 'community', $2, 'troll', 999)",
    )
    .bind(10007_i64)
    .bind(community_id)
    .execute(&pool)
    .await?;

    let global_entries =
        verifier_bot::db::BlacklistRepo::find_by_telegram_user_id(&pool, 10006)
            .await
            .expect("find blacklist");
    assert!(global_entries
        .iter()
        .any(|e| e.scope_type == verifier_bot::domain::ScopeType::Global));

    let community_entries =
        verifier_bot::db::BlacklistRepo::find_by_telegram_user_id(&pool, 10007)
            .await
            .expect("find blacklist");
    assert!(community_entries.iter().any(|e| {
        e.scope_type == verifier_bot::domain::ScopeType::Community
            && e.community_id == Some(community_id)
    }));

    let clean_entries =
        verifier_bot::db::BlacklistRepo::find_by_telegram_user_id(&pool, 99999)
            .await
            .expect("find blacklist");
    assert!(clean_entries.is_empty());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn expiry_recent_requests_not_affected(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1001000000007, "expiry-test-7").await;
    let applicant_id = seed_applicant(&pool, 10008).await;
    let jr_id = seed_join_request(
        &pool,
        community_id,
        applicant_id,
        10008,
        "questionnaire_in_progress",
        30,
    )
    .await;

    let api = FakeTelegramApi::new();
    process_tick(&api, &pool, &default_settings())
        .await
        .expect("process_tick");

    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM join_requests WHERE id = $1")
            .bind(jr_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "questionnaire_in_progress");

    assert!(api.sent_messages().is_empty());
    assert!(api.declined_requests().is_empty());

    Ok(())
}
