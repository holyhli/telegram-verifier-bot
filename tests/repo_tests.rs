use chrono::Utc;
use sqlx::PgPool;
use verifier_bot::db::{
    AnswerRepo, ApplicantRepo, BlacklistRepo, CommunityRepo, JoinRequestRepo,
    ModerationActionRepo, SessionRepo,
};
use verifier_bot::domain::{ActionType, JoinRequestStatus, ScopeType, SessionState};
use verifier_bot::error::AppError;

async fn seed_community(pool: &PgPool) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO communities (telegram_chat_id, title, slug)
         VALUES (-1009999999999, 'Test Community', 'test-repo')
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("seed community");
    id
}

async fn seed_community_question(pool: &PgPool, community_id: i64, position: i32) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO community_questions (community_id, question_key, question_text, required, position)
         VALUES ($1, $2, $3, TRUE, $4) RETURNING id",
    )
    .bind(community_id)
    .bind(format!("q{position}"))
    .bind(format!("Question {position}?"))
    .bind(position)
    .fetch_one(pool)
    .await
    .expect("seed question");
    id
}

async fn seed_applicant(pool: &PgPool, telegram_user_id: i64) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO applicants (telegram_user_id, first_name)
         VALUES ($1, 'TestUser') RETURNING id",
    )
    .bind(telegram_user_id)
    .fetch_one(pool)
    .await
    .expect("seed applicant");
    id
}

// --- Status Transition Tests ---

#[test]
fn status_transition_pending_to_questionnaire() {
    assert!(JoinRequestStatus::PendingContact
        .can_transition_to(&JoinRequestStatus::QuestionnaireInProgress));
}

#[test]
fn status_transition_questionnaire_to_submitted() {
    assert!(JoinRequestStatus::QuestionnaireInProgress
        .can_transition_to(&JoinRequestStatus::Submitted));
}

#[test]
fn status_transition_submitted_to_approved() {
    assert!(JoinRequestStatus::Submitted.can_transition_to(&JoinRequestStatus::Approved));
}

#[test]
fn status_transition_submitted_to_rejected() {
    assert!(JoinRequestStatus::Submitted.can_transition_to(&JoinRequestStatus::Rejected));
}

#[test]
fn status_transition_submitted_to_banned() {
    assert!(JoinRequestStatus::Submitted.can_transition_to(&JoinRequestStatus::Banned));
}

#[test]
fn status_transition_any_active_to_expired() {
    assert!(JoinRequestStatus::PendingContact.can_transition_to(&JoinRequestStatus::Expired));
    assert!(
        JoinRequestStatus::QuestionnaireInProgress
            .can_transition_to(&JoinRequestStatus::Expired)
    );
    assert!(JoinRequestStatus::Submitted.can_transition_to(&JoinRequestStatus::Expired));
}

#[test]
fn status_transition_any_active_to_cancelled() {
    assert!(JoinRequestStatus::PendingContact.can_transition_to(&JoinRequestStatus::Cancelled));
    assert!(
        JoinRequestStatus::QuestionnaireInProgress
            .can_transition_to(&JoinRequestStatus::Cancelled)
    );
    assert!(JoinRequestStatus::Submitted.can_transition_to(&JoinRequestStatus::Cancelled));
}

#[test]
fn status_transition_invalid_backwards() {
    assert!(!JoinRequestStatus::Submitted
        .can_transition_to(&JoinRequestStatus::PendingContact));
    assert!(!JoinRequestStatus::Approved
        .can_transition_to(&JoinRequestStatus::Submitted));
}

#[test]
fn status_transition_terminal_cannot_transition() {
    assert!(!JoinRequestStatus::Approved.can_transition_to(&JoinRequestStatus::Expired));
    assert!(!JoinRequestStatus::Rejected.can_transition_to(&JoinRequestStatus::Cancelled));
    assert!(!JoinRequestStatus::Banned.can_transition_to(&JoinRequestStatus::Approved));
    assert!(!JoinRequestStatus::Expired.can_transition_to(&JoinRequestStatus::PendingContact));
    assert!(!JoinRequestStatus::Cancelled.can_transition_to(&JoinRequestStatus::Submitted));
}

#[test]
fn status_is_terminal() {
    assert!(JoinRequestStatus::Approved.is_terminal());
    assert!(JoinRequestStatus::Rejected.is_terminal());
    assert!(JoinRequestStatus::Banned.is_terminal());
    assert!(JoinRequestStatus::Expired.is_terminal());
    assert!(JoinRequestStatus::Cancelled.is_terminal());
    assert!(!JoinRequestStatus::PendingContact.is_terminal());
    assert!(!JoinRequestStatus::QuestionnaireInProgress.is_terminal());
    assert!(!JoinRequestStatus::Submitted.is_terminal());
}

// --- Community Repo Tests ---

#[sqlx::test(migrations = "./migrations")]
async fn community_find_by_telegram_chat_id(pool: PgPool) -> sqlx::Result<()> {
    seed_community(&pool).await;

    let found = CommunityRepo::find_by_telegram_chat_id(&pool, -1009999999999)
        .await
        .expect("find community");
    assert!(found.is_some());
    assert_eq!(found.as_ref().unwrap().slug, "test-repo");

    let not_found = CommunityRepo::find_by_telegram_chat_id(&pool, -99999)
        .await
        .expect("find missing");
    assert!(not_found.is_none());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn community_find_active_questions(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    seed_community_question(&pool, community_id, 1).await;
    seed_community_question(&pool, community_id, 2).await;

    sqlx::query(
        "INSERT INTO community_questions (community_id, question_key, question_text, required, position, is_active)
         VALUES ($1, 'inactive', 'Inactive?', TRUE, 3, FALSE)"
    )
    .bind(community_id)
    .execute(&pool)
    .await?;

    let questions = CommunityRepo::find_active_questions(&pool, community_id)
        .await
        .expect("find questions");
    assert_eq!(questions.len(), 2);
    assert_eq!(questions[0].position, 1);
    assert_eq!(questions[1].position, 2);

    Ok(())
}

// --- Applicant Repo Tests ---

#[sqlx::test(migrations = "./migrations")]
async fn applicant_find_or_create_idempotent(pool: PgPool) -> sqlx::Result<()> {
    let a1 = ApplicantRepo::find_or_create_by_telegram_user_id(
        &pool, 42000, "Alice", Some("Smith"), Some("alice_s"),
    )
    .await
    .expect("create applicant");

    let a2 = ApplicantRepo::find_or_create_by_telegram_user_id(
        &pool, 42000, "Alice Updated", None, Some("alice_new"),
    )
    .await
    .expect("upsert applicant");

    assert_eq!(a1.id, a2.id);
    assert_eq!(a2.first_name, "Alice Updated");
    assert!(a2.last_name.is_none());
    assert_eq!(a2.username.as_deref(), Some("alice_new"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn applicant_update_profile(pool: PgPool) -> sqlx::Result<()> {
    let applicant = ApplicantRepo::find_or_create_by_telegram_user_id(
        &pool, 42001, "Bob", None, None,
    )
    .await
    .expect("create");

    let updated = ApplicantRepo::update_profile(
        &pool, applicant.id, "Robert", Some("Jones"), Some("bob_j"),
    )
    .await
    .expect("update");

    assert_eq!(updated.first_name, "Robert");
    assert_eq!(updated.last_name.as_deref(), Some("Jones"));

    Ok(())
}

// --- JoinRequest Repo Tests ---

#[sqlx::test(migrations = "./migrations")]
async fn join_request_create_and_find(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 50000).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 99999, Utc::now())
        .await
        .expect("create join request");

    assert_eq!(jr.status, JoinRequestStatus::PendingContact);
    assert_eq!(jr.community_id, community_id);

    let found = JoinRequestRepo::find_by_id(&pool, jr.id)
        .await
        .expect("find");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, jr.id);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn join_request_find_active_for_applicant(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 50001).await;

    JoinRequestRepo::create(&pool, community_id, applicant_id, 99998, Utc::now())
        .await
        .expect("create");

    let active = JoinRequestRepo::find_active_for_applicant_in_community(
        &pool,
        applicant_id,
        community_id,
    )
    .await
    .expect("find active");
    assert!(active.is_some());

    let none = JoinRequestRepo::find_active_for_applicant_in_community(&pool, applicant_id, 99999)
        .await
        .expect("find none");
    assert!(none.is_none());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn join_request_update_status_valid_transition(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 50002).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 99997, Utc::now())
        .await
        .expect("create");

    let updated = JoinRequestRepo::update_status(
        &pool,
        jr.id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::QuestionnaireInProgress,
        jr.updated_at,
    )
    .await
    .expect("transition");

    assert_eq!(updated.status, JoinRequestStatus::QuestionnaireInProgress);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn join_request_update_status_invalid_transition(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 50003).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 99996, Utc::now())
        .await
        .expect("create");

    let result = JoinRequestRepo::update_status(
        &pool,
        jr.id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::Approved,
        jr.updated_at,
    )
    .await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AppError::InvalidStateTransition { .. }
    ));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn join_request_optimistic_locking_conflict(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 50004).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 99995, Utc::now())
        .await
        .expect("create");

    let stale_updated_at = jr.updated_at;

    // First update succeeds
    JoinRequestRepo::update_status(
        &pool,
        jr.id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::QuestionnaireInProgress,
        stale_updated_at,
    )
    .await
    .expect("first update");

    // Second update with stale timestamp fails (optimistic lock)
    let result = JoinRequestRepo::update_status(
        &pool,
        jr.id,
        JoinRequestStatus::PendingContact,
        JoinRequestStatus::QuestionnaireInProgress,
        stale_updated_at,
    )
    .await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AppError::AlreadyProcessed { .. }
    ));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn join_request_find_expired(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 50005).await;

    sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date, created_at)
         VALUES ($1, $2, 88880, 'pending_contact', NOW() - interval '2 hours', NOW() - interval '2 hours')"
    )
    .bind(community_id)
    .bind(applicant_id)
    .execute(&pool)
    .await?;

    let cutoff = Utc::now() - chrono::Duration::hours(1);
    let expired = JoinRequestRepo::find_expired(&pool, cutoff)
        .await
        .expect("find expired");
    assert_eq!(expired.len(), 1);

    let cutoff_old = Utc::now() - chrono::Duration::hours(3);
    let none = JoinRequestRepo::find_expired(&pool, cutoff_old)
        .await
        .expect("find none expired");
    assert!(none.is_empty());

    Ok(())
}

// --- Answer Repo Tests ---

#[sqlx::test(migrations = "./migrations")]
async fn answer_create_and_find(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let q_id = seed_community_question(&pool, community_id, 1).await;
    let applicant_id = seed_applicant(&pool, 60000).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 77770, Utc::now())
        .await
        .expect("create jr");

    let answer = AnswerRepo::create(&pool, jr.id, q_id, "My answer text")
        .await
        .expect("create answer");
    assert_eq!(answer.answer_text, "My answer text");

    let answers = AnswerRepo::find_by_join_request_id(&pool, jr.id)
        .await
        .expect("find answers");
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].community_question_id, q_id);

    Ok(())
}

// --- Moderation Repo Tests ---

#[sqlx::test(migrations = "./migrations")]
async fn moderation_create_and_find(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 70000).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 66660, Utc::now())
        .await
        .expect("create jr");

    let action = ModerationActionRepo::create(&pool, jr.id, 999888, ActionType::Approved)
        .await
        .expect("create action");
    assert_eq!(action.action_type, ActionType::Approved);
    assert_eq!(action.moderator_telegram_user_id, 999888);

    let actions = ModerationActionRepo::find_by_join_request_id(&pool, jr.id)
        .await
        .expect("find actions");
    assert_eq!(actions.len(), 1);

    Ok(())
}

// --- Blacklist Repo Tests ---

#[sqlx::test(migrations = "./migrations")]
async fn blacklist_global_scope(pool: PgPool) -> sqlx::Result<()> {
    let entry = BlacklistRepo::create(
        &pool, 80000, ScopeType::Global, None, Some("spam"), 111222,
    )
    .await
    .expect("create blacklist");

    assert_eq!(entry.scope_type, ScopeType::Global);
    assert!(entry.community_id.is_none());

    let entries = BlacklistRepo::find_by_telegram_user_id(&pool, 80000)
        .await
        .expect("find");
    assert_eq!(entries.len(), 1);

    let empty = BlacklistRepo::find_by_telegram_user_id(&pool, 99999)
        .await
        .expect("find empty");
    assert!(empty.is_empty());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn blacklist_community_scope(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;

    let entry = BlacklistRepo::create(
        &pool, 80001, ScopeType::Community, Some(community_id), Some("trolling"), 111333,
    )
    .await
    .expect("create community blacklist");

    assert_eq!(entry.scope_type, ScopeType::Community);
    assert_eq!(entry.community_id, Some(community_id));

    Ok(())
}

// --- Session Repo Tests ---

#[sqlx::test(migrations = "./migrations")]
async fn session_lifecycle(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 90000).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 55550, Utc::now())
        .await
        .expect("create jr");

    let session = SessionRepo::create(&pool, jr.id, 1)
        .await
        .expect("create session");
    assert_eq!(session.state, SessionState::AwaitingAnswer);
    assert_eq!(session.current_question_position, 1);

    let found = SessionRepo::find_active_by_join_request_id(&pool, jr.id)
        .await
        .expect("find active");
    assert!(found.is_some());

    let advanced = SessionRepo::advance_question(&pool, session.id, 2)
        .await
        .expect("advance");
    assert_eq!(advanced.current_question_position, 2);

    let completed = SessionRepo::complete(&pool, session.id)
        .await
        .expect("complete");
    assert_eq!(completed.state, SessionState::Completed);

    let no_active = SessionRepo::find_active_by_join_request_id(&pool, jr.id)
        .await
        .expect("no active after complete");
    assert!(no_active.is_none());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn session_expire(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let applicant_id = seed_applicant(&pool, 90001).await;

    let jr = JoinRequestRepo::create(&pool, community_id, applicant_id, 55551, Utc::now())
        .await
        .expect("create jr");

    let session = SessionRepo::create(&pool, jr.id, 1)
        .await
        .expect("create session");

    let expired = SessionRepo::expire(&pool, session.id)
        .await
        .expect("expire");
    assert_eq!(expired.state, SessionState::Expired);

    Ok(())
}

// --- find_needing_reminder Test ---

#[sqlx::test(migrations = "./migrations")]
async fn join_request_find_needing_reminder(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let a1 = seed_applicant(&pool, 50010).await;
    let a2 = seed_applicant(&pool, 50011).await;

    sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date, created_at)
         VALUES ($1, $2, 88810, 'questionnaire_in_progress', NOW() - interval '50 minutes', NOW() - interval '50 minutes')"
    )
    .bind(community_id)
    .bind(a1)
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date, created_at, reminder_sent_at)
         VALUES ($1, $2, 88811, 'questionnaire_in_progress', NOW() - interval '50 minutes', NOW() - interval '50 minutes', NOW())"
    )
    .bind(community_id)
    .bind(a2)
    .execute(&pool)
    .await?;

    let cutoff = Utc::now() - chrono::Duration::minutes(45);
    let needing = JoinRequestRepo::find_needing_reminder(&pool, cutoff)
        .await
        .expect("find needing reminder");

    assert_eq!(needing.len(), 1);
    assert_eq!(needing[0].applicant_id, a1);

    Ok(())
}
