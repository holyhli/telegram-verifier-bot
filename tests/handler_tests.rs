use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use teloxide::ApiError;
use teloxide::RequestError;
use verifier_bot::bot::handlers::join_request::{process_join_request, JoinRequestInput};
use verifier_bot::bot::handlers::start::{process_start, StartInput};
use verifier_bot::bot::handlers::TelegramApi;
use verifier_bot::db::{JoinRequestRepo, SessionRepo};
use verifier_bot::domain::JoinRequestStatus;

#[derive(Debug, Clone)]
struct FakeTelegramApi {
    sent_messages: Arc<Mutex<Vec<(i64, String)>>>,
    declined_requests: Arc<Mutex<Vec<(i64, i64)>>>,
    send_error: Arc<Mutex<Option<RequestError>>>,
}

impl FakeTelegramApi {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            declined_requests: Arc::new(Mutex::new(Vec::new())),
            send_error: Arc::new(Mutex::new(None)),
        }
    }

    fn with_send_error(err: RequestError) -> Self {
        let api = Self::new();
        *api.send_error.lock().expect("lock send_error") = Some(err);
        api
    }

    fn sent_messages(&self) -> Vec<(i64, String)> {
        self.sent_messages
            .lock()
            .expect("lock sent_messages")
            .clone()
    }

    fn declined_requests(&self) -> Vec<(i64, i64)> {
        self.declined_requests
            .lock()
            .expect("lock declined_requests")
            .clone()
    }
}

#[async_trait]
impl TelegramApi for FakeTelegramApi {
    async fn send_message(&self, chat_id: i64, text: String) -> Result<(), RequestError> {
        if let Some(err) = self.send_error.lock().expect("lock send_error").clone() {
            return Err(err);
        }

        self.sent_messages
            .lock()
            .expect("lock sent_messages")
            .push((chat_id, text));
        Ok(())
    }

    async fn decline_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError> {
        self.declined_requests
            .lock()
            .expect("lock declined_requests")
            .push((chat_id, user_id));
        Ok(())
    }
}

async fn seed_community(pool: &PgPool, chat_id: i64, slug: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO communities (telegram_chat_id, title, slug) VALUES ($1, 'Community A', $2) RETURNING id",
    )
    .bind(chat_id)
    .bind(slug)
    .fetch_one(pool)
    .await
    .expect("seed community");
    id
}

async fn seed_question(pool: &PgPool, community_id: i64, text: &str) {
    sqlx::query(
        "INSERT INTO community_questions (community_id, question_key, question_text, required, position)
         VALUES ($1, 'q1', $2, TRUE, 1)",
    )
    .bind(community_id)
    .bind(text)
    .execute(pool)
    .await
    .expect("seed question");
}

async fn seed_blacklist_global(pool: &PgPool, telegram_user_id: i64) {
    sqlx::query(
        "INSERT INTO blacklist_entries (telegram_user_id, scope_type, community_id, reason, created_by_moderator_id)
         VALUES ($1, 'global', NULL, 'spam', 111)",
    )
    .bind(telegram_user_id)
    .execute(pool)
    .await
    .expect("seed blacklist");
}

fn sample_join_request_input(community_chat_id: i64) -> JoinRequestInput {
    JoinRequestInput {
        community_chat_id,
        community_title: "Community A".to_string(),
        telegram_user_id: 123_456,
        user_chat_id: 123_456,
        first_name: "Alice".to_string(),
        last_name: Some("Smith".to_string()),
        username: Some("alice".to_string()),
        join_request_date: Utc::now(),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn handle_join_request_creates_join_request_session_and_sends_message(
    pool: PgPool,
) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000001, "handler-test-1").await;
    seed_question(&pool, community_id, "What do you build?").await;

    let api = FakeTelegramApi::new();
    process_join_request(&api, &pool, sample_join_request_input(-1009000000001))
        .await
        .expect("process join request");

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].1.contains("What do you build?"));

    let join_requests: Vec<(String,)> = sqlx::query_as("SELECT status::text FROM join_requests")
        .fetch_all(&pool)
        .await?;
    assert_eq!(join_requests.len(), 1);
    assert_eq!(join_requests[0].0, "questionnaire_in_progress");

    let session_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM applicant_sessions")
        .fetch_one(&pool)
        .await?;
    assert_eq!(session_count.0, 1);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn handle_join_request_unknown_community_does_not_change_db(
    pool: PgPool,
) -> sqlx::Result<()> {
    let api = FakeTelegramApi::new();
    process_join_request(&api, &pool, sample_join_request_input(-1001111111111))
        .await
        .expect("unknown community should be ignored");

    let applicant_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM applicants")
        .fetch_one(&pool)
        .await?;
    let join_request_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM join_requests")
        .fetch_one(&pool)
        .await?;

    assert_eq!(applicant_count.0, 0);
    assert_eq!(join_request_count.0, 0);
    assert!(api.sent_messages().is_empty());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn handle_join_request_blacklisted_user_declined(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000002, "handler-test-2").await;
    seed_question(&pool, community_id, "Why join?").await;
    seed_blacklist_global(&pool, 123_456).await;

    let api = FakeTelegramApi::new();
    process_join_request(&api, &pool, sample_join_request_input(-1009000000002))
        .await
        .expect("blacklisted request should be declined");

    assert_eq!(api.declined_requests(), vec![(-1009000000002, 123_456)]);

    let join_request_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM join_requests")
        .fetch_one(&pool)
        .await?;
    assert_eq!(join_request_count.0, 0);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn handle_join_request_duplicate_update_is_idempotent(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000003, "handler-test-3").await;
    seed_question(&pool, community_id, "What is your stack?").await;
    let api = FakeTelegramApi::new();

    process_join_request(&api, &pool, sample_join_request_input(-1009000000003))
        .await
        .expect("first request");

    process_join_request(&api, &pool, sample_join_request_input(-1009000000003))
        .await
        .expect("duplicate request");

    let join_request_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM join_requests")
        .fetch_one(&pool)
        .await?;
    assert_eq!(join_request_count.0, 1);
    assert_eq!(api.sent_messages().len(), 1);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn start_with_pending_contact_resumes_questionnaire(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000004, "handler-test-4").await;
    seed_question(&pool, community_id, "Where are you based?").await;

    let applicant_id: (i64,) = sqlx::query_as(
        "INSERT INTO applicants (telegram_user_id, first_name) VALUES ($1, 'Bob') RETURNING id",
    )
    .bind(222_333_i64)
    .fetch_one(&pool)
    .await?;

    let join_request =
        JoinRequestRepo::create(&pool, community_id, applicant_id.0, 222_333, Utc::now())
            .await
            .expect("create pending join request");
    assert_eq!(join_request.status, JoinRequestStatus::PendingContact);

    let api = FakeTelegramApi::new();
    process_start(
        &api,
        &pool,
        StartInput {
            user_chat_id: 222_333,
            telegram_user_id: 222_333,
            first_name: "Bob".to_string(),
        },
    )
    .await
    .expect("resume via /start");

    assert_eq!(api.sent_messages().len(), 1);
    let updated = JoinRequestRepo::find_by_id(&pool, join_request.id)
        .await
        .expect("load join request")
        .expect("join request must exist");
    assert_eq!(updated.status, JoinRequestStatus::QuestionnaireInProgress);
    assert!(
        SessionRepo::find_active_by_join_request_id(&pool, join_request.id)
            .await
            .expect("find session")
            .is_some()
    );

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn start_without_pending_request_sends_generic_message(pool: PgPool) -> sqlx::Result<()> {
    let api = FakeTelegramApi::new();
    process_start(
        &api,
        &pool,
        StartInput {
            user_chat_id: 777_888,
            telegram_user_id: 777_888,
            first_name: "Charlie".to_string(),
        },
    )
    .await
    .expect("generic /start should succeed");

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert!(sent[0]
        .1
        .contains("If you've requested to join a community"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn join_request_blocked_user_marks_request_cancelled(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000005, "handler-test-5").await;
    seed_question(&pool, community_id, "Tell us about yourself").await;

    let api = FakeTelegramApi::with_send_error(RequestError::Api(ApiError::BotBlocked));
    process_join_request(&api, &pool, sample_join_request_input(-1009000000005))
        .await
        .expect("blocked user path should not fail handler");

    let statuses: Vec<(String,)> = sqlx::query_as("SELECT status::text FROM join_requests")
        .fetch_all(&pool)
        .await?;
    assert_eq!(statuses, vec![("cancelled".to_string(),)]);

    Ok(())
}
