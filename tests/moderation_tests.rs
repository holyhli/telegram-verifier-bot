use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use teloxide::{ApiError, RequestError};
use teloxide::types::InlineKeyboardMarkup;

use verifier_bot::bot::handlers::callbacks::{
    parse_callback_data, process_callback_query, CallbackAction, CallbackActionInput,
};
use verifier_bot::bot::handlers::TelegramApi;
use verifier_bot::config::{BotSettings, Config};
use verifier_bot::db::{AnswerRepo, JoinRequestRepo, ModerationActionRepo};
use verifier_bot::domain::{JoinRequestStatus, ScopeType};
use verifier_bot::services::moderator::{
    render_moderator_card, send_moderator_card, ModeratorCardAnswer, ModeratorCardContext,
};

#[derive(Debug, Clone)]
struct FakeTelegramApi {
    sent_messages: Arc<Mutex<Vec<(i64, String)>>>,
    sent_html_messages: Arc<Mutex<Vec<(i64, String, Option<InlineKeyboardMarkup>)>>>,
    edited_messages: Arc<Mutex<Vec<(i64, i64, String)>>>,
    cleared_markup: Arc<Mutex<Vec<(i64, i64)>>>,
    answered_callbacks: Arc<Mutex<Vec<(String, String)>>>,
    approved_requests: Arc<Mutex<Vec<(i64, i64)>>>,
    declined_requests: Arc<Mutex<Vec<(i64, i64)>>>,
    send_error: Arc<Mutex<Option<RequestError>>>,
    approve_error: Arc<Mutex<Option<RequestError>>>,
    edited_messages_with_markup: Arc<Mutex<Vec<(i64, i32, String, Option<Vec<Vec<(String, String)>>>)>>>,
    decline_error: Arc<Mutex<Option<RequestError>>>,
}

impl FakeTelegramApi {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            sent_html_messages: Arc::new(Mutex::new(Vec::new())),
            edited_messages: Arc::new(Mutex::new(Vec::new())),
            cleared_markup: Arc::new(Mutex::new(Vec::new())),
            answered_callbacks: Arc::new(Mutex::new(Vec::new())),
            approved_requests: Arc::new(Mutex::new(Vec::new())),
            declined_requests: Arc::new(Mutex::new(Vec::new())),
            send_error: Arc::new(Mutex::new(None)),
            approve_error: Arc::new(Mutex::new(None)),
            decline_error: Arc::new(Mutex::new(None)),
            edited_messages_with_markup: Arc::new(Mutex::new(vec![])),
        }
    }

    fn with_send_error(err: RequestError) -> Self {
        let api = Self::new();
        *api.send_error.lock().expect("lock send_error") = Some(err);
        api
    }

    fn with_approve_error(err: RequestError) -> Self {
        let api = Self::new();
        *api.approve_error.lock().expect("lock approve_error") = Some(err);
        api
    }

    fn answered_callbacks(&self) -> Vec<(String, String)> {
        self.answered_callbacks
            .lock()
            .expect("lock answered_callbacks")
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

    async fn send_message_html(
        &self,
        chat_id: i64,
        text: String,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<i64, RequestError> {
        let mut messages = self
            .sent_html_messages
            .lock()
            .expect("lock sent_html_messages");
        messages.push((chat_id, text, reply_markup));
        Ok(messages.len() as i64)
    }

    async fn edit_message_html(
        &self,
        chat_id: i64,
        message_id: i64,
        text: String,
    ) -> Result<(), RequestError> {
        self.edited_messages
            .lock()
            .expect("lock edited_messages")
            .push((chat_id, message_id, text));
        Ok(())
    }

    async fn clear_message_reply_markup(
        &self,
        chat_id: i64,
        message_id: i64,
    ) -> Result<(), RequestError> {
        self.cleared_markup
            .lock()
            .expect("lock cleared_markup")
            .push((chat_id, message_id));
        Ok(())
    }

    async fn answer_callback_query(
        &self,
        callback_query_id: String,
        text: String,
    ) -> Result<(), RequestError> {
        self.answered_callbacks
            .lock()
            .expect("lock answered_callbacks")
            .push((callback_query_id, text));
        Ok(())
    }

    async fn approve_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError> {
        if let Some(err) = self
            .approve_error
            .lock()
            .expect("lock approve_error")
            .clone()
        {
            return Err(err);
        }

        self.approved_requests
            .lock()
            .expect("lock approved_requests")
            .push((chat_id, user_id));
        Ok(())
    }

    async fn decline_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError> {
        if let Some(err) = self
            .decline_error
            .lock()
            .expect("lock decline_error")
            .clone()
        {
            return Err(err);
        }

        self.declined_requests
            .lock()
            .expect("lock declined_requests")
            .push((chat_id, user_id));
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
            .expect("lock edited_messages_with_markup")
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

fn test_config(allowed_moderator_ids: Vec<i64>, default_moderator_chat_id: i64) -> Config {
    Config {
        bot_token: "token".to_string(),
        database_url: "postgres://example".to_string(),
        default_moderator_chat_id,
        allowed_moderator_ids,
        use_webhooks: false,
        public_webhook_url: None,
        server_port: 8080,
        rust_log: "info".to_string(),
        bot_settings: BotSettings::default(),
        communities: vec![],
    }
}

async fn seed_community(pool: &PgPool, chat_id: i64, slug: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO communities (telegram_chat_id, title, slug) VALUES ($1, 'Moderation Community', $2) RETURNING id",
    )
    .bind(chat_id)
    .bind(slug)
    .fetch_one(pool)
    .await
    .expect("seed community");
    id
}

async fn seed_question(pool: &PgPool, community_id: i64, key: &str, text: &str, position: i32) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO community_questions (community_id, question_key, question_text, required, position)
         VALUES ($1, $2, $3, TRUE, $4) RETURNING id",
    )
    .bind(community_id)
    .bind(key)
    .bind(text)
    .bind(position)
    .fetch_one(pool)
    .await
    .expect("seed question");
    id
}

async fn seed_applicant(pool: &PgPool, telegram_user_id: i64, username: Option<&str>) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO applicants (telegram_user_id, first_name, last_name, username)
         VALUES ($1, 'Alicia', 'Tester', $2)
         RETURNING id",
    )
    .bind(telegram_user_id)
    .bind(username)
    .fetch_one(pool)
    .await
    .expect("seed applicant");
    id
}

async fn seed_submitted_join_request(pool: &PgPool, username: Option<&str>, suffix: i64) -> i64 {
    let community_id = seed_community(pool, -100_990_000_0000 - suffix, &format!("moderation-{suffix}"))
        .await;
    let q1 = seed_question(pool, community_id, "q1", "Why do you want to join?", 1).await;
    let q2 = seed_question(pool, community_id, "q2", "How will you contribute?", 2).await;

    let telegram_user_id = 555_000 + suffix;
    let applicant_id = seed_applicant(pool, telegram_user_id, username).await;
    let join_request = JoinRequestRepo::create(
        pool,
        community_id,
        applicant_id,
        telegram_user_id,
        Utc::now(),
    )
    .await
    .expect("create join request");

    let jr_progress = JoinRequestRepo::update_status(
        pool,
        join_request.id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::QuestionnaireInProgress,
        join_request.updated_at,
    )
    .await
    .expect("transition to questionnaire_in_progress");

    let jr_submitted = JoinRequestRepo::update_status(
        pool,
        jr_progress.id,
        JoinRequestStatus::QuestionnaireInProgress,
        JoinRequestStatus::Submitted,
        jr_progress.updated_at,
    )
    .await
    .expect("transition to submitted");

    AnswerRepo::create(pool, jr_submitted.id, q1, "I want to collaborate and learn.")
        .await
        .expect("seed answer 1");
    AnswerRepo::create(pool, jr_submitted.id, q2, "I can help with moderation tooling.")
        .await
        .expect("seed answer 2");

    jr_submitted.id
}

fn callback_input(moderator_id: i64, action: CallbackAction, join_request_id: i64) -> CallbackActionInput {
    let action_char = match action {
        CallbackAction::Approve => "a",
        CallbackAction::Reject => "r",
        CallbackAction::Ban => "b",
    };

    CallbackActionInput {
        callback_query_id: format!("cb-{join_request_id}-{moderator_id}"),
        callback_data: Some(format!("{action_char}:{join_request_id}")),
        moderator_telegram_user_id: moderator_id,
        message_chat_id: Some(-100_123_000_111),
        message_id: Some(77),
    }
}

#[test]
fn moderation_render_card_includes_all_fields() {
    let context = ModeratorCardContext {
        join_request_id: 42,
        community_title: "Rustaceans".to_string(),
        community_chat_id: -100111,
        applicant_first_name: "Ada".to_string(),
        applicant_last_name: Some("Lovelace".to_string()),
        applicant_username: Some("ada".to_string()),
        applicant_telegram_user_id: 1001,
        applicant_chat_id: 1001,
        join_request_date: Utc::now(),
        completed_at: Utc::now(),
    };
    let answers = vec![ModeratorCardAnswer {
        position: 1,
        question_text: "Why?".to_string(),
        answer_text: "To contribute".to_string(),
    }];

    let card = render_moderator_card(&context, &answers);
    assert!(card.contains("<b>📋 New Join Request</b>"));
    assert!(card.contains("<b>Community:</b> Rustaceans"));
    assert!(card.contains("<b>Applicant:</b> Ada Lovelace"));
    assert!(card.contains("<b>Username:</b> @ada"));
    assert!(card.contains("<b>Telegram ID:</b> <code>1001</code>"));
    assert!(card.contains("1. <b>Why?:</b> To contribute"));
    assert!(card.contains("<b>Status:</b> Submitted"));
    assert!(card.contains("<b>Request ID:</b> <code>42</code>"));
}

#[test]
fn moderation_render_card_username_not_set() {
    let context = ModeratorCardContext {
        join_request_id: 1,
        community_title: "Rust".to_string(),
        community_chat_id: -100,
        applicant_first_name: "No".to_string(),
        applicant_last_name: Some("Username".to_string()),
        applicant_username: None,
        applicant_telegram_user_id: 999,
        applicant_chat_id: 999,
        join_request_date: Utc::now(),
        completed_at: Utc::now(),
    };

    let card = render_moderator_card(&context, &[]);
    assert!(card.contains("<b>Username:</b> not set"));
}

#[test]
fn moderation_parse_callback_data_valid() {
    assert_eq!(parse_callback_data("a:10"), Some((CallbackAction::Approve, 10)));
    assert_eq!(parse_callback_data("r:20"), Some((CallbackAction::Reject, 20)));
    assert_eq!(parse_callback_data("b:30"), Some((CallbackAction::Ban, 30)));
}

#[test]
fn moderation_parse_callback_data_invalid() {
    assert_eq!(parse_callback_data("x:10"), None);
    assert_eq!(parse_callback_data("a:not-a-number"), None);
    assert_eq!(parse_callback_data("broken"), None);
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_send_card_delivers_html_and_keyboard(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 1).await;
    let api = FakeTelegramApi::new();

    send_moderator_card(&api, &pool, join_request_id, -100_123_000_111)
        .await
        .expect("send moderator card");

    let sent = api
        .sent_html_messages
        .lock()
        .expect("lock sent_html_messages")
        .clone();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].1.contains("<b>📋 New Join Request</b>"));
    let keyboard_debug = format!("{:?}", sent[0].2.clone().expect("keyboard present"));
    assert!(keyboard_debug.contains(&format!("a:{join_request_id}")));
    assert!(keyboard_debug.contains(&format!("r:{join_request_id}")));
    assert!(keyboard_debug.contains(&format!("b:{join_request_id}")));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_authorization_denied(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 2).await;
    let api = FakeTelegramApi::new();
    let config = test_config(vec![111], -100_123_000_111);

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(999, CallbackAction::Approve, join_request_id),
    )
    .await
    .expect("callback should be handled");

    let callbacks = api.answered_callbacks();
    assert_eq!(callbacks.len(), 1);
    assert_eq!(callbacks[0].1, "You are not authorized");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_approve_flow_updates_status_audit_and_card(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 3).await;
    let api = FakeTelegramApi::new();
    send_moderator_card(&api, &pool, join_request_id, -100_123_000_111)
        .await
        .expect("send card");
    let config = test_config(vec![5001], -100_123_000_111);

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(5001, CallbackAction::Approve, join_request_id),
    )
    .await
    .expect("approve flow");

    let updated = JoinRequestRepo::find_by_id(&pool, join_request_id)
        .await
        .expect("find join request")
        .expect("join request exists");
    assert_eq!(updated.status, JoinRequestStatus::Approved);
    assert!(updated.approved_at.is_some());

    let actions = ModerationActionRepo::find_by_join_request_id(&pool, join_request_id)
        .await
        .expect("find moderation actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_type.to_string(), "approved");

    assert_eq!(
        api.approved_requests
            .lock()
            .expect("lock approved_requests")
            .len(),
        1
    );
    assert_eq!(
        api.edited_messages
            .lock()
            .expect("lock edited_messages")
            .len(),
        1
    );
    assert_eq!(
        api.cleared_markup
            .lock()
            .expect("lock cleared_markup")
            .len(),
        1
    );

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_reject_flow_updates_status_audit_and_card(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 4).await;
    let api = FakeTelegramApi::new();
    let config = test_config(vec![5002], -100_123_000_111);

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(5002, CallbackAction::Reject, join_request_id),
    )
    .await
    .expect("reject flow");

    let updated = JoinRequestRepo::find_by_id(&pool, join_request_id)
        .await
        .expect("find join request")
        .expect("join request exists");
    assert_eq!(updated.status, JoinRequestStatus::Rejected);
    assert!(updated.rejected_at.is_some());

    let actions = ModerationActionRepo::find_by_join_request_id(&pool, join_request_id)
        .await
        .expect("find moderation actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_type.to_string(), "rejected");

    assert_eq!(
        api.declined_requests
            .lock()
            .expect("lock declined_requests")
            .len(),
        1
    );

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_ban_flow_updates_status_audit_blacklist(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 5).await;
    let api = FakeTelegramApi::new();
    let config = test_config(vec![5003], -100_123_000_111);

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(5003, CallbackAction::Ban, join_request_id),
    )
    .await
    .expect("ban flow");

    let updated = JoinRequestRepo::find_by_id(&pool, join_request_id)
        .await
        .expect("find join request")
        .expect("join request exists");
    assert_eq!(updated.status, JoinRequestStatus::Banned);

    let actions = ModerationActionRepo::find_by_join_request_id(&pool, join_request_id)
        .await
        .expect("find moderation actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_type.to_string(), "banned");

    let scope_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT scope_type::text FROM blacklist_entries ORDER BY id ASC",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(scope_rows.len(), 1);
    assert_eq!(scope_rows[0].0, ScopeType::Community.to_string());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_double_processing_returns_friendly_error(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 6).await;
    let api = FakeTelegramApi::new();
    let config = test_config(vec![5004, 5005], -100_123_000_111);

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(5004, CallbackAction::Approve, join_request_id),
    )
    .await
    .expect("first moderation action");

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(5005, CallbackAction::Reject, join_request_id),
    )
    .await
    .expect("second moderation action should be handled");

    let callbacks = api.answered_callbacks();
    assert!(callbacks
        .iter()
        .any(|(_, text)| text == "Already processed by another moderator"));

    let actions = ModerationActionRepo::find_by_join_request_id(&pool, join_request_id)
        .await
        .expect("find moderation actions");
    assert_eq!(actions.len(), 1);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_hide_requester_missing_is_handled(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 7).await;
    let api = FakeTelegramApi::with_approve_error(RequestError::Api(ApiError::Unknown(
        "Bad Request: HIDE_REQUESTER_MISSING".to_string(),
    )));
    let config = test_config(vec![5006], -100_123_000_111);

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(5006, CallbackAction::Approve, join_request_id),
    )
    .await
    .expect("hide requester missing should be handled");

    let callbacks = api.answered_callbacks();
    assert!(callbacks
        .iter()
        .any(|(_, text)| text == "Request already processed outside bot"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn moderation_applicant_blocked_does_not_fail(pool: PgPool) -> sqlx::Result<()> {
    let join_request_id = seed_submitted_join_request(&pool, Some("alice"), 8).await;
    let api = FakeTelegramApi::with_send_error(RequestError::Api(ApiError::BotBlocked));
    let config = test_config(vec![5007], -100_123_000_111);

    process_callback_query(
        &api,
        &pool,
        &config,
        callback_input(5007, CallbackAction::Reject, join_request_id),
    )
    .await
    .expect("applicant blocked should not fail flow");

    let updated = JoinRequestRepo::find_by_id(&pool, join_request_id)
        .await
        .expect("find join request")
        .expect("join request exists");
    assert_eq!(updated.status, JoinRequestStatus::Rejected);

    Ok(())
}
