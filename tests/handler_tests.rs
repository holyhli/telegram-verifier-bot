use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use teloxide::ApiError;
use teloxide::RequestError;
use teloxide::types::InlineKeyboardMarkup;
use verifier_bot::bot::handlers::join_request::{process_join_request, JoinRequestInput};
use verifier_bot::bot::handlers::language_selection::process_language_selection_callback;
use verifier_bot::bot::handlers::start::{process_start, StartInput};
use verifier_bot::bot::handlers::TelegramApi;
use verifier_bot::db::{JoinRequestRepo, SessionRepo};
use verifier_bot::domain::{JoinRequestStatus, Language};

#[derive(Debug, Clone)]
struct FakeTelegramApi {
    sent_messages: Arc<Mutex<Vec<(i64, String)>>>,
    keyboards_sent: Arc<Mutex<Vec<(i64, String, Vec<Vec<(String, String)>>)>>>,
    declined_requests: Arc<Mutex<Vec<(i64, i64)>>>,
    send_error: Arc<Mutex<Option<RequestError>>>,
}

impl FakeTelegramApi {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            keyboards_sent: Arc::new(Mutex::new(Vec::new())),
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

    fn keyboards_sent(&self) -> Vec<(i64, String, Vec<Vec<(String, String)>>)> {
        self.keyboards_sent
            .lock()
            .expect("lock keyboards_sent")
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

    async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: String,
        keyboard: Vec<Vec<(String, String)>>,
    ) -> Result<(), RequestError> {
        if let Some(err) = self.send_error.lock().expect("lock send_error").clone() {
            return Err(err);
        }

        self.keyboards_sent
            .lock()
            .expect("lock keyboards_sent")
            .push((chat_id, text.clone(), keyboard));
        self.sent_messages
            .lock()
            .expect("lock sent_messages")
            .push((chat_id, text));
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
        "INSERT INTO community_questions (community_id, question_key, question_text, question_text_uk, required, position)
         VALUES ($1, 'q1', $2, $3, TRUE, 1)",
    )
    .bind(community_id)
    .bind(text)
    .bind(format!("{} (Ukrainian)", text))
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
async fn handle_join_request_sends_language_selection(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000001, "handler-test-1").await;
    seed_question(&pool, community_id, "What do you build?").await;

    let api = FakeTelegramApi::new();
    process_join_request(&api, &pool, sample_join_request_input(-1009000000001))
        .await
        .expect("process join request");

    // Verify keyboard was sent
    let keyboards = api.keyboards_sent();
    assert_eq!(keyboards.len(), 1);
    let (chat_id, text, keyboard) = &keyboards[0];
    assert_eq!(*chat_id, 123_456);
    assert!(text.contains("Alice"));
    assert!(text.contains("Community A"));

    // Verify keyboard structure
    assert_eq!(keyboard.len(), 1); // One row
    assert_eq!(keyboard[0].len(), 2); // Two buttons
    assert_eq!(keyboard[0][0].0, "🇬🇧 English");
    assert_eq!(keyboard[0][0].1, "lang:en");
    assert_eq!(keyboard[0][1].0, "🇺🇦 Українська");
    assert_eq!(keyboard[0][1].1, "lang:uk");

    // Verify join request created but status remains PendingContact
    let join_requests: Vec<(String,)> = sqlx::query_as("SELECT status::text FROM join_requests")
        .fetch_all(&pool)
        .await?;
    assert_eq!(join_requests.len(), 1);
    assert_eq!(join_requests[0].0, "pending_contact");

    // Verify NO session created yet
    let session_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM applicant_sessions")
        .fetch_one(&pool)
        .await?;
    assert_eq!(session_count.0, 0);

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

#[sqlx::test(migrations = "./migrations")]
async fn language_selection_en_creates_session_and_sends_question(
    pool: PgPool,
) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000010, "lang-test-1").await;
    seed_question(&pool, community_id, "What do you build?").await;

    let applicant_id: i64 = sqlx::query_scalar(
        "INSERT INTO applicants (telegram_user_id, first_name) VALUES (123456, 'Alice') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;

    let join_request_id: i64 = sqlx::query_scalar(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, telegram_join_request_date, status)
         VALUES ($1, $2, 123456, NOW(), 'pending_contact') RETURNING id",
    )
    .bind(community_id)
    .bind(applicant_id)
    .fetch_one(&pool)
    .await?;

    let api = FakeTelegramApi::new();
    process_language_selection_callback(
        &api,
        &pool,
        "callback_123".to_string(),
        123456,
        123456,
        "lang:en".to_string(),
    )
    .await
    .expect("language selection should succeed");

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, 123456);
    assert!(sent[0].1.contains("Hi Alice!"));
    assert!(sent[0].1.contains("What do you build?"));
    assert!(!sent[0].1.contains("Ukrainian"));

    let session = SessionRepo::find_active_by_join_request_id(&pool, join_request_id)
        .await
        .expect("find session")
        .expect("session should exist");
    assert_eq!(session.current_question_position, 1);
    assert_eq!(session.language, Language::English);

    let jr = JoinRequestRepo::find_by_id(&pool, join_request_id)
        .await
        .expect("find join request")
        .expect("join request should exist");
    assert_eq!(jr.status, JoinRequestStatus::QuestionnaireInProgress);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn language_selection_uk_creates_session_and_sends_question(
    pool: PgPool,
) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000011, "lang-test-2").await;
    seed_question(&pool, community_id, "What do you build?").await;

    let applicant_id: i64 = sqlx::query_scalar(
        "INSERT INTO applicants (telegram_user_id, first_name) VALUES (234567, 'Олена') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;

    let join_request_id: i64 = sqlx::query_scalar(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, telegram_join_request_date, status)
         VALUES ($1, $2, 234567, NOW(), 'pending_contact') RETURNING id",
    )
    .bind(community_id)
    .bind(applicant_id)
    .fetch_one(&pool)
    .await?;

    let api = FakeTelegramApi::new();
    process_language_selection_callback(
        &api,
        &pool,
        "callback_456".to_string(),
        234567,
        234567,
        "lang:uk".to_string(),
    )
    .await
    .expect("language selection should succeed");

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, 234567);
    assert!(sent[0].1.contains("Привіт, Олена!"));
    assert!(sent[0].1.contains("What do you build? (Ukrainian)"));

    let session = SessionRepo::find_active_by_join_request_id(&pool, join_request_id)
        .await
        .expect("find session")
        .expect("session should exist");
    assert_eq!(session.current_question_position, 1);
    assert_eq!(session.language, Language::Ukrainian);

    let jr = JoinRequestRepo::find_by_id(&pool, join_request_id)
        .await
        .expect("find join request")
        .expect("join request should exist");
    assert_eq!(jr.status, JoinRequestStatus::QuestionnaireInProgress);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn language_selection_invalid_code_returns_error(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000012, "lang-test-3").await;
    seed_question(&pool, community_id, "Question").await;

    let applicant_id: i64 = sqlx::query_scalar(
        "INSERT INTO applicants (telegram_user_id, first_name) VALUES (345678, 'Bob') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, telegram_join_request_date, status)
         VALUES ($1, $2, 345678, NOW(), 'pending_contact')",
    )
    .bind(community_id)
    .bind(applicant_id)
    .execute(&pool)
    .await?;

    let api = FakeTelegramApi::new();
    let result = process_language_selection_callback(
        &api,
        &pool,
        "callback_789".to_string(),
        345678,
        345678,
        "lang:fr".to_string(),
    )
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown language code"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn language_selection_no_join_request_returns_error(pool: PgPool) -> sqlx::Result<()> {
    let api = FakeTelegramApi::new();
    let result = process_language_selection_callback(
        &api,
        &pool,
        "callback_999".to_string(),
        999999,
        999999,
        "lang:en".to_string(),
    )
    .await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No active join request found"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn language_selection_wrong_status_returns_error(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool, -1009000000013, "lang-test-4").await;
    seed_question(&pool, community_id, "Question").await;

    let applicant_id: i64 = sqlx::query_scalar(
        "INSERT INTO applicants (telegram_user_id, first_name) VALUES (456789, 'Charlie') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, telegram_join_request_date, status)
         VALUES ($1, $2, 456789, NOW(), 'submitted')",
    )
    .bind(community_id)
    .bind(applicant_id)
    .execute(&pool)
    .await?;

    let api = FakeTelegramApi::new();
    let result = process_language_selection_callback(
        &api,
        &pool,
        "callback_111".to_string(),
        456789,
        456789,
        "lang:en".to_string(),
    )
    .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Invalid state transition") || err_msg.contains("submitted"));

    Ok(())
}
