use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use teloxide::RequestError;
use teloxide::types::InlineKeyboardMarkup;
use verifier_bot::bot::handlers::join_request::{process_join_request, JoinRequestInput};
use verifier_bot::bot::handlers::language_selection::process_language_selection_callback;
use verifier_bot::bot::handlers::questionnaire::{process_private_message, PrivateMessageInput};
use verifier_bot::bot::handlers::TelegramApi;
use verifier_bot::db::{CommunityRepo, JoinRequestRepo, SessionRepo};
use verifier_bot::domain::{JoinRequestStatus, Language};
use verifier_bot::messages::Messages;

#[derive(Debug, Clone)]
struct FakeTelegramApi {
    sent_messages: Arc<Mutex<Vec<(i64, String)>>>,
    keyboards_sent: Arc<Mutex<Vec<(i64, String, Vec<Vec<(String, String)>>)>>>,
    edited_messages_with_markup: Arc<Mutex<Vec<(i64, i32, String, Option<Vec<Vec<(String, String)>>>)>>>,
}

impl FakeTelegramApi {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            keyboards_sent: Arc::new(Mutex::new(Vec::new())),
            edited_messages_with_markup: Arc::new(Mutex::new(vec![])),
        }
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

    async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: String,
        keyboard: Vec<Vec<(String, String)>>,
    ) -> Result<(), RequestError> {
        self.keyboards_sent
            .lock()
            .expect("lock keyboards_sent")
            .push((chat_id, text, keyboard));
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
}

// Test helper: seed community with bilingual questions
async fn seed_community_with_questions(pool: &PgPool) -> i64 {
    let community_id = sqlx::query_scalar!(
        r#"
        INSERT INTO communities (telegram_chat_id, title, slug, created_at, updated_at)
        VALUES (-1001234567890, 'Test Community', 'test-community', NOW(), NOW())
        RETURNING id
        "#
    )
    .fetch_one(pool)
    .await
    .expect("insert community");

    sqlx::query!(
        r#"
        INSERT INTO community_questions (community_id, question_key, question_text, question_text_uk, required, position, created_at, updated_at)
        VALUES 
            ($1, 'name', 'What is your name?', 'Як вас звати?', true, 1, NOW(), NOW()),
            ($1, 'occupation', 'What do you do?', 'Чим ви займаєтесь?', true, 2, NOW(), NOW()),
            ($1, 'referral', 'How did you hear about us?', 'Як ви про нас дізналися?', false, 3, NOW(), NOW())
        "#,
        community_id
    )
    .execute(pool)
    .await
    .expect("insert questions");

    community_id
}

// Test helper: seed applicant
async fn seed_applicant(pool: &PgPool, telegram_user_id: i64, first_name: &str) -> i64 {
    sqlx::query_scalar!(
        r#"
        INSERT INTO applicants (telegram_user_id, first_name, username, created_at, updated_at)
        VALUES ($1, $2, 'testuser', NOW(), NOW())
        RETURNING id
        "#,
        telegram_user_id,
        first_name
    )
    .fetch_one(pool)
    .await
    .expect("insert applicant")
}

// Test helper: seed join request
async fn seed_join_request(
    pool: &PgPool,
    applicant_id: i64,
    community_id: i64,
    telegram_user_chat_id: i64,
    status: &str,
) -> i64 {
    sqlx::query_scalar!(
        r#"
        INSERT INTO join_requests (applicant_id, community_id, telegram_user_chat_id, status, telegram_join_request_date, created_at, updated_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW(), NOW())
        RETURNING id
        "#,
        applicant_id,
        community_id,
        telegram_user_chat_id,
        status
    )
    .fetch_one(pool)
    .await
    .expect("insert join request")
}

#[sqlx::test]
async fn language_selection_full_flow_english(pool: PgPool) {
    // Setup
    let community_id = seed_community_with_questions(&pool).await;
    let telegram_user_id = 123456789i64;
    let applicant_id = seed_applicant(&pool, telegram_user_id, "Alice").await;
    let join_request_id = seed_join_request(&pool, applicant_id, community_id, telegram_user_id, "pending_contact").await;

    let api = FakeTelegramApi::new();

    // Step 1: Process language selection callback (English)
    process_language_selection_callback(
        &api,
        &pool,
        "callback_123".to_string(),
        telegram_user_id,
        telegram_user_id,
        "lang:en".to_string(),
    )
    .await
    .expect("process language selection");

    // Verify session created with English language
    let session = sqlx::query!(
        r#"
        SELECT language as "language: Language", current_question_position
        FROM applicant_sessions
        WHERE join_request_id = $1
        "#,
        join_request_id
    )
    .fetch_one(&pool)
    .await
    .expect("fetch session");

    assert_eq!(session.language, Language::English);
    assert_eq!(session.current_question_position, 2);

    // Verify join request status transitioned to questionnaire_in_progress
    let join_request = sqlx::query!(
        r#"
        SELECT status as "status: JoinRequestStatus"
        FROM join_requests
        WHERE id = $1
        "#,
        join_request_id
    )
    .fetch_one(&pool)
    .await
    .expect("fetch join request");

    assert_eq!(join_request.status, JoinRequestStatus::QuestionnaireInProgress);

    // Verify welcome message and first question sent in English
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 1);
    let (chat_id, text) = &messages[0];
    assert_eq!(*chat_id, telegram_user_id);
    assert!(text.contains("Hi Alice!"));
    assert!(text.contains("What do you do?"));
    assert!(!text.contains("Чим ви займаєтесь?"));

    // Step 2: Answer second question (first question was answered via name prompt)
    process_private_message(
        &api,
        &pool,
        PrivateMessageInput {
            chat_id: telegram_user_id,
            telegram_user_id,
            text: "Software Engineer".to_string(),
        },
        1234567890i64,
    )
    .await
    .expect("process answer");

    // Verify third question sent in English
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 2);
    let (_, text) = &messages[1];
    assert!(text.contains("How did you hear about us?"));
    assert!(!text.contains("Як ви про нас дізналися?"));
    // Step 3: Answer third question
    process_private_message(
        &api,
        &pool,
        PrivateMessageInput {
            chat_id: telegram_user_id,
            telegram_user_id,
            text: "From a friend".to_string(),
        },
        1234567890i64,
    )
    .await
    .expect("process answer");

    // Verify completion message sent in English
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 4);
    let (_, text) = &messages[2];
    let expected_completion = Messages::completion_message(Language::English);
    assert!(text.contains(&expected_completion));
    assert!(!text.contains("Дякуємо"));

    // Verify join request status transitioned to submitted
    let join_request = sqlx::query!(
        r#"
        SELECT status as "status: JoinRequestStatus"
        FROM join_requests
        WHERE id = $1
        "#,
        join_request_id
    )
    .fetch_one(&pool)
    .await
    .expect("fetch join request");

    assert_eq!(join_request.status, JoinRequestStatus::Submitted);

    // Verify all answers stored
    let answers = sqlx::query!(
        r#"
        SELECT cq.question_key, jra.answer_text
        FROM join_request_answers jra
        JOIN community_questions cq ON jra.community_question_id = cq.id
        WHERE jra.join_request_id = $1
        ORDER BY cq.question_key
        "#,
        join_request_id
    )
    .fetch_all(&pool)
    .await
    .expect("fetch answers");

    assert_eq!(answers.len(), 3);
    assert_eq!(answers[0].question_key, "name");
    assert_eq!(answers[0].answer_text, "Alice");
    assert_eq!(answers[1].question_key, "occupation");
    assert_eq!(answers[1].answer_text, "Software Engineer");
    assert_eq!(answers[2].question_key, "referral");
    assert_eq!(answers[2].answer_text, "From a friend");
}

#[sqlx::test]
async fn language_selection_full_flow_ukrainian(pool: PgPool) {
    // Setup
    let community_id = seed_community_with_questions(&pool).await;
    let telegram_user_id = 987654321i64;
    let applicant_id = seed_applicant(&pool, telegram_user_id, "Богдан").await;
    let join_request_id = seed_join_request(&pool, applicant_id, community_id, telegram_user_id, "pending_contact").await;

    let api = FakeTelegramApi::new();

    // Step 1: Process language selection callback (Ukrainian)
    process_language_selection_callback(
        &api,
        &pool,
        "callback_456".to_string(),
        telegram_user_id,
        telegram_user_id,
        "lang:uk".to_string(),
    )
    .await
    .expect("process language selection");

    // Verify session created with Ukrainian language
    let session = sqlx::query!(
        r#"
        SELECT language as "language: Language", current_question_position
        FROM applicant_sessions
        WHERE join_request_id = $1
        "#,
        join_request_id
    )
    .fetch_one(&pool)
    .await
    .expect("fetch session");

    assert_eq!(session.language, Language::Ukrainian);
    assert_eq!(session.current_question_position, 2);

    // Verify join request status transitioned to questionnaire_in_progress
    let join_request = sqlx::query!(
        r#"
        SELECT status as "status: JoinRequestStatus"
        FROM join_requests
        WHERE id = $1
        "#,
        join_request_id
    )
    .fetch_one(&pool)
    .await
    .expect("fetch join request");

    assert_eq!(join_request.status, JoinRequestStatus::QuestionnaireInProgress);

    // Verify welcome message and first question sent in Ukrainian
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 1);
    let (chat_id, text) = &messages[0];
    assert_eq!(*chat_id, telegram_user_id);
    assert!(text.contains("Привіт, Богдан!"));
    assert!(text.contains("Чим ви займаєтесь?"));
    assert!(!text.contains("What do you do?"));

    // Step 2: Answer second question (first question was answered via name prompt)
    process_private_message(
        &api,
        &pool,
        PrivateMessageInput {
            chat_id: telegram_user_id,
            telegram_user_id,
            text: "Розробник програмного забезпечення".to_string(),
        },
        1234567890i64,
    )
    .await
    .expect("process answer");

    // Verify third question sent in Ukrainian
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 2);
    let (_, text) = &messages[1];
    assert!(text.contains("Як ви про нас дізналися?"));
    assert!(!text.contains("How did you hear about us?"));
    // Step 3: Answer third question
    process_private_message(
        &api,
        &pool,
        PrivateMessageInput {
            chat_id: telegram_user_id,
            telegram_user_id,
            text: "Від друга".to_string(),
        },
        1234567890i64,
    )
    .await
    .expect("process answer");

    // Verify completion message sent in Ukrainian
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 4);
    let (_, text) = &messages[2];
    let expected_completion = Messages::completion_message(Language::Ukrainian);
    assert!(text.contains(&expected_completion));
    assert!(text.contains("Дяку"));
    assert!(!text.contains("Thank you"));

    // Verify join request status transitioned to submitted
    let join_request = sqlx::query!(
        r#"
        SELECT status as "status: JoinRequestStatus"
        FROM join_requests
        WHERE id = $1
        "#,
        join_request_id
    )
    .fetch_one(&pool)
    .await
    .expect("fetch join request");

    assert_eq!(join_request.status, JoinRequestStatus::Submitted);

    // Verify all answers stored
    let answers = sqlx::query!(
        r#"
        SELECT cq.question_key, jra.answer_text
        FROM join_request_answers jra
        JOIN community_questions cq ON jra.community_question_id = cq.id
        WHERE jra.join_request_id = $1
        ORDER BY cq.question_key
        "#,
        join_request_id
    )
    .fetch_all(&pool)
    .await
    .expect("fetch answers");

    assert_eq!(answers.len(), 3);
    assert_eq!(answers[0].question_key, "name");
    assert_eq!(answers[0].answer_text, "Богдан");
    assert_eq!(answers[1].question_key, "occupation");
    assert_eq!(answers[1].answer_text, "Розробник програмного забезпечення");
    assert_eq!(answers[2].question_key, "referral");
    assert_eq!(answers[2].answer_text, "Від друга");
}

#[sqlx::test]
async fn language_selection_validation_errors_respect_language(pool: PgPool) {
    // Setup
    let community_id = seed_community_with_questions(&pool).await;
    let telegram_user_id = 111222333i64;
    let applicant_id = seed_applicant(&pool, telegram_user_id, "Олена").await;
    let join_request_id = seed_join_request(&pool, applicant_id, community_id, telegram_user_id, "pending_contact").await;

    let api = FakeTelegramApi::new();

    // Step 1: Select Ukrainian language
    process_language_selection_callback(
        &api,
        &pool,
        "callback_789".to_string(),
        telegram_user_id,
        telegram_user_id,
        "lang:uk".to_string(),
    )
    .await
    .expect("process language selection");

    // Step 2: Try to submit empty answer (required field)
    process_private_message(
        &api,
        &pool,
        PrivateMessageInput {
            chat_id: telegram_user_id,
            telegram_user_id,
            text: "".to_string(),
        },
        1234567890i64,
    )
    .await
    .expect("process empty answer");

    // Verify error message is in Ukrainian
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 2); // Welcome + error
    let (_, error_text) = &messages[1];
    let expected_error = Messages::required_field_error(Language::Ukrainian);
    assert_eq!(error_text, &expected_error);
    assert!(error_text.contains("Це поле обов'язкове"));
    assert!(!error_text.contains("This field is required"));

    // Step 3: Try to submit too short answer
    process_private_message(
        &api,
        &pool,
        PrivateMessageInput {
            chat_id: telegram_user_id,
            telegram_user_id,
            text: "A".to_string(),
        },
        1234567890i64,
    )
    .await
    .expect("process short answer");

    // Verify error message is in Ukrainian
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 3); // Welcome + error + error
    let (_, error_text) = &messages[2];
    let expected_error = Messages::min_length_error(Language::Ukrainian);
    assert_eq!(error_text, &expected_error);
    assert!(error_text.contains("Твоя відповідь має містити хоча б 2 символи"));
    assert!(!error_text.contains("Your answer must be at least 2 characters long."));

    // Step 4: Try to submit low effort answer
    process_private_message(
        &api,
        &pool,
        PrivateMessageInput {
            chat_id: telegram_user_id,
            telegram_user_id,
            text: "aaa".to_string(),
        },
        1234567890i64,
    )
    .await
    .expect("process low effort answer");

    // Verify error message is in Ukrainian
    let messages = api.sent_messages();
    assert_eq!(messages.len(), 4); // Welcome + error + error + error
    let (_, error_text) = &messages[3];
    let expected_error = Messages::low_effort_error(Language::Ukrainian);
    assert_eq!(error_text, &expected_error);
    assert!(error_text.contains("Будь ласка, дай більш детальну відповідь"));
    assert!(!error_text.contains("Please provide a more detailed answer."));
}
