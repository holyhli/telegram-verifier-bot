use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use teloxide::RequestError;
use teloxide::types::InlineKeyboardMarkup;
use verifier_bot::bot::handlers::questionnaire::{process_private_message, PrivateMessageInput};
use verifier_bot::bot::handlers::TelegramApi;
use verifier_bot::db::{JoinRequestRepo, SessionRepo};
use verifier_bot::domain::JoinRequestStatus;
use verifier_bot::services::questionnaire::validate_answer;

#[derive(Debug, Clone)]
struct FakeTelegramApi {
    sent_messages: Arc<Mutex<Vec<(i64, String)>>>,
}

impl FakeTelegramApi {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn sent_messages(&self) -> Vec<(i64, String)> {
        self.sent_messages
            .lock()
            .expect("lock sent_messages")
            .clone()
    }
}

#[async_trait]
impl TelegramApi for FakeTelegramApi {
    async fn send_message(&self, chat_id: i64, text: String) -> Result<(), RequestError> {
        self.sent_messages
            .lock()
            .expect("lock sent_messages")
            .push((chat_id, text));
        Ok(())
    }

    async fn decline_chat_join_request(
        &self,
        _chat_id: i64,
        _user_id: i64,
    ) -> Result<(), RequestError> {
        Ok(())
    }

    async fn send_message_html(
        &self,
        chat_id: i64,
        text: String,
        _reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<i64, RequestError> {
        self.sent_messages
            .lock()
            .expect("lock sent_messages")
            .push((chat_id, text));
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
}

async fn seed_community(pool: &PgPool, chat_id: i64, slug: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO communities (telegram_chat_id, title, slug) VALUES ($1, 'Questionnaire Community', $2) RETURNING id",
    )
    .bind(chat_id)
    .bind(slug)
    .fetch_one(pool)
    .await
    .expect("seed community");
    id
}

async fn seed_questions(pool: &PgPool, community_id: i64, count: i32, required: bool) {
    for position in 1..=count {
        sqlx::query(
            "INSERT INTO community_questions (community_id, question_key, question_text, required, position)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(community_id)
        .bind(format!("q{position}"))
        .bind(format!("Question {position}?"))
        .bind(required)
        .bind(position)
        .execute(pool)
        .await
        .expect("seed question");
    }
}

async fn seed_applicant(pool: &PgPool, telegram_user_id: i64) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO applicants (telegram_user_id, first_name) VALUES ($1, 'Taylor') RETURNING id",
    )
    .bind(telegram_user_id)
    .fetch_one(pool)
    .await
    .expect("seed applicant");
    id
}

async fn seed_active_questionnaire(
    pool: &PgPool,
    telegram_user_id: i64,
    question_count: i32,
) -> i64 {
    let community_id = seed_community(
        pool,
        -100_990_000_0000 - telegram_user_id,
        &format!("questionnaire-{telegram_user_id}"),
    )
    .await;
    seed_questions(pool, community_id, question_count, true).await;

    let applicant_id = seed_applicant(pool, telegram_user_id).await;
    let join_request = JoinRequestRepo::create(pool, community_id, applicant_id, telegram_user_id, Utc::now())
        .await
        .expect("create join request");

    let updated = JoinRequestRepo::update_status(
        pool,
        join_request.id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::QuestionnaireInProgress,
        join_request.updated_at,
    )
    .await
    .expect("transition to questionnaire_in_progress");

    SessionRepo::create(pool, updated.id, 1)
        .await
        .expect("create active session");

    updated.id
}

fn sample_private_message_input(telegram_user_id: i64, text: &str) -> PrivateMessageInput {
    PrivateMessageInput {
        chat_id: telegram_user_id,
        telegram_user_id,
        text: text.to_string(),
    }
}

#[test]
fn questionnaire_validate_answer_required_valid() {
    let result = validate_answer("I build moderation bots", true);
    assert!(result.is_ok());
    assert_eq!(result.expect("valid answer"), "I build moderation bots");
}

#[test]
fn questionnaire_validate_answer_required_empty_rejected() {
    let result = validate_answer("", true);
    assert_eq!(
        result.expect_err("empty answer must fail").to_string(),
        "This question is required. Please provide an answer."
    );
}

#[test]
fn questionnaire_validate_answer_required_whitespace_rejected() {
    let result = validate_answer("   \n\t", true);
    assert_eq!(
        result.expect_err("whitespace answer must fail").to_string(),
        "This question is required. Please provide an answer."
    );
}

#[test]
fn questionnaire_validate_answer_required_too_short_rejected() {
    let result = validate_answer("a", true);
    assert_eq!(
        result.expect_err("one-char answer must fail").to_string(),
        "Please provide a more detailed answer (at least a few words)."
    );
}

#[test]
fn questionnaire_validate_answer_low_effort_rejected_case_insensitive() {
    let result = validate_answer("TeSt", true);
    assert_eq!(
        result
            .expect_err("low-effort answer must fail")
            .to_string(),
        "Please provide a genuine answer so moderators can review your application."
    );
}

#[test]
fn questionnaire_validate_answer_optional_accepts_empty() {
    let result = validate_answer("   ", false);
    assert!(result.is_ok());
    assert_eq!(result.expect("optional empty allowed"), "");
}

#[sqlx::test(migrations = "./migrations")]
async fn questionnaire_process_answer_stores_and_advances(pool: PgPool) -> sqlx::Result<()> {
    let telegram_user_id = 55_001;
    let join_request_id = seed_active_questionnaire(&pool, telegram_user_id, 2).await;
    let api = FakeTelegramApi::new();

    process_private_message(
        &api,
        &pool,
        sample_private_message_input(telegram_user_id, "I am an active contributor."),
        -100_123,
    )
    .await
    .expect("process private message");

    let (answer_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM join_request_answers WHERE join_request_id = $1")
            .bind(join_request_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(answer_count, 1);

    let (position,): (i32,) =
        sqlx::query_as("SELECT current_question_position FROM applicant_sessions")
            .fetch_one(&pool)
            .await?;
    assert_eq!(position, 2);

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, "Question 2?");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn questionnaire_validation_failure_does_not_advance_or_store(pool: PgPool) -> sqlx::Result<()> {
    let telegram_user_id = 55_002;
    let join_request_id = seed_active_questionnaire(&pool, telegram_user_id, 2).await;
    let api = FakeTelegramApi::new();

    process_private_message(
        &api,
        &pool,
        sample_private_message_input(telegram_user_id, "test"),
        -100_123,
    )
        .await
        .expect("validation failure should be handled");

    let (answer_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM join_request_answers WHERE join_request_id = $1")
            .bind(join_request_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(answer_count, 0);

    let (position,): (i32,) =
        sqlx::query_as("SELECT current_question_position FROM applicant_sessions")
            .fetch_one(&pool)
            .await?;
    assert_eq!(position, 1);

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].1,
        "Please provide a genuine answer so moderators can review your application."
    );

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn questionnaire_process_answer_completes_on_last_question(pool: PgPool) -> sqlx::Result<()> {
    let telegram_user_id = 55_003;
    let join_request_id = seed_active_questionnaire(&pool, telegram_user_id, 1).await;
    let api = FakeTelegramApi::new();

    process_private_message(
        &api,
        &pool,
        sample_private_message_input(telegram_user_id, "I can help others and share knowledge."),
        -100_123,
    )
    .await
    .expect("last question should complete flow");

    let (session_state,): (String,) = sqlx::query_as("SELECT state::text FROM applicant_sessions")
        .fetch_one(&pool)
        .await?;
    assert_eq!(session_state, "completed");

    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM join_requests WHERE id = $1")
            .bind(join_request_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "submitted");

    let sent = api.sent_messages();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[0].1,
        "Thanks — your application has been submitted to the moderators.\nYou'll be notified once a decision is made."
    );
    assert!(sent[1].1.contains("<b>📋 New Join Request</b>"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn questionnaire_full_five_question_flow_submits(pool: PgPool) -> sqlx::Result<()> {
    let telegram_user_id = 55_004;
    let join_request_id = seed_active_questionnaire(&pool, telegram_user_id, 5).await;
    let api = FakeTelegramApi::new();

    for idx in 1..=5 {
        process_private_message(
            &api,
            &pool,
            sample_private_message_input(
                telegram_user_id,
                &format!("This is a detailed answer number {idx}."),
            ),
            -100_123,
        )
        .await
        .expect("flow step must succeed");
    }

    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM join_requests WHERE id = $1")
            .bind(join_request_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(status, "submitted");

    let (answers,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM join_request_answers WHERE join_request_id = $1")
            .bind(join_request_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(answers, 5);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn questionnaire_out_of_order_no_session_is_ignored(pool: PgPool) -> sqlx::Result<()> {
    let api = FakeTelegramApi::new();

    process_private_message(
        &api,
        &pool,
        sample_private_message_input(99_999, "Hello?"),
        -100_123,
    )
        .await
        .expect("no active session should be ignored");

    let (answer_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM join_request_answers")
        .fetch_one(&pool)
        .await?;
    assert_eq!(answer_count, 0);
    assert!(api.sent_messages().is_empty());

    Ok(())
}
