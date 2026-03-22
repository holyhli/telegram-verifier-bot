use sqlx::PgPool;
use verifier_bot::config::{CommunityConfig, Question};
use verifier_bot::db::sync::sync_config_to_db;

#[sqlx::test]
async fn migrations_apply_to_fresh_db(pool: PgPool) -> sqlx::Result<()> {
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
    )
    .fetch_all(&pool)
    .await?;

    let table_names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();

    assert!(table_names.contains(&"communities"));
    assert!(table_names.contains(&"community_questions"));
    assert!(table_names.contains(&"applicants"));
    assert!(table_names.contains(&"join_requests"));
    assert!(table_names.contains(&"join_request_answers"));
    assert!(table_names.contains(&"moderation_actions"));
    assert!(table_names.contains(&"blacklist_entries"));
    assert!(table_names.contains(&"applicant_sessions"));

    Ok(())
}

#[sqlx::test]
async fn sync_creates_communities_and_questions(pool: PgPool) -> sqlx::Result<()> {
    let communities = vec![CommunityConfig {
        telegram_chat_id: -1001111111111,
        title: "Test Community".into(),
        slug: "test-community".into(),
        questions: vec![
            Question {
                key: "name".into(),
                text: "What is your name?".into(),
                required: true,
                position: 1,
            },
            Question {
                key: "reason".into(),
                text: "Why do you want to join?".into(),
                required: false,
                position: 2,
            },
        ],
    }];

    sync_config_to_db(&pool, &communities).await.unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM communities")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count.0, 1);

    let q_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM community_questions")
        .fetch_one(&pool)
        .await?;
    assert_eq!(q_count.0, 2);

    let (title,): (String,) =
        sqlx::query_as("SELECT title FROM communities WHERE slug = 'test-community'")
            .fetch_one(&pool)
            .await?;
    assert_eq!(title, "Test Community");

    Ok(())
}

#[sqlx::test]
async fn sync_updates_existing_community(pool: PgPool) -> sqlx::Result<()> {
    let initial = vec![CommunityConfig {
        telegram_chat_id: -1002222222222,
        title: "Old Title".into(),
        slug: "update-test".into(),
        questions: vec![Question {
            key: "q1".into(),
            text: "Old question?".into(),
            required: true,
            position: 1,
        }],
    }];

    sync_config_to_db(&pool, &initial).await.unwrap();

    let updated = vec![CommunityConfig {
        telegram_chat_id: -1002222222222,
        title: "New Title".into(),
        slug: "update-test".into(),
        questions: vec![Question {
            key: "q1".into(),
            text: "Updated question?".into(),
            required: false,
            position: 1,
        }],
    }];

    sync_config_to_db(&pool, &updated).await.unwrap();

    let (title,): (String,) =
        sqlx::query_as("SELECT title FROM communities WHERE telegram_chat_id = -1002222222222")
            .fetch_one(&pool)
            .await?;
    assert_eq!(title, "New Title");

    let (text, required): (String, bool) = sqlx::query_as(
        "SELECT question_text, required FROM community_questions WHERE question_key = 'q1'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(text, "Updated question?");
    assert!(!required);

    let community_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM communities")
        .fetch_one(&pool)
        .await?;
    assert_eq!(community_count.0, 1);

    Ok(())
}

#[sqlx::test]
async fn sync_deactivates_removed_questions(pool: PgPool) -> sqlx::Result<()> {
    let initial = vec![CommunityConfig {
        telegram_chat_id: -1003333333333,
        title: "Deactivation Test".into(),
        slug: "deactivate-test".into(),
        questions: vec![
            Question {
                key: "keep".into(),
                text: "Keep this?".into(),
                required: true,
                position: 1,
            },
            Question {
                key: "remove".into(),
                text: "Remove this?".into(),
                required: true,
                position: 2,
            },
        ],
    }];

    sync_config_to_db(&pool, &initial).await.unwrap();

    let updated = vec![CommunityConfig {
        telegram_chat_id: -1003333333333,
        title: "Deactivation Test".into(),
        slug: "deactivate-test".into(),
        questions: vec![Question {
            key: "keep".into(),
            text: "Keep this?".into(),
            required: true,
            position: 1,
        }],
    }];

    sync_config_to_db(&pool, &updated).await.unwrap();

    let (active,): (bool,) = sqlx::query_as(
        "SELECT is_active FROM community_questions WHERE question_key = 'remove'",
    )
    .fetch_one(&pool)
    .await?;
    assert!(!active);

    let (still_active,): (bool,) =
        sqlx::query_as("SELECT is_active FROM community_questions WHERE question_key = 'keep'")
            .fetch_one(&pool)
            .await?;
    assert!(still_active);

    Ok(())
}

#[sqlx::test]
async fn duplicate_active_join_request_rejected(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO communities (telegram_chat_id, title, slug) VALUES (-100999, 'C', 'c')")
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO applicants (telegram_user_id, first_name) VALUES (12345, 'Alice')")
        .execute(&pool)
        .await?;

    let community_id: (i64,) =
        sqlx::query_as("SELECT id FROM communities WHERE slug = 'c'")
            .fetch_one(&pool)
            .await?;
    let applicant_id: (i64,) =
        sqlx::query_as("SELECT id FROM applicants WHERE telegram_user_id = 12345")
            .fetch_one(&pool)
            .await?;

    sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date) \
         VALUES ($1, $2, 99999, 'pending_contact', NOW())"
    )
    .bind(community_id.0)
    .bind(applicant_id.0)
    .execute(&pool)
    .await?;

    let result = sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date) \
         VALUES ($1, $2, 88888, 'questionnaire_in_progress', NOW())"
    )
    .bind(community_id.0)
    .bind(applicant_id.0)
    .execute(&pool)
    .await;

    assert!(result.is_err());

    Ok(())
}

#[sqlx::test]
async fn invalid_status_rejected_by_check_constraint(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO communities (telegram_chat_id, title, slug) VALUES (-100888, 'C2', 'c2')")
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO applicants (telegram_user_id, first_name) VALUES (67890, 'Bob')")
        .execute(&pool)
        .await?;

    let community_id: (i64,) =
        sqlx::query_as("SELECT id FROM communities WHERE slug = 'c2'")
            .fetch_one(&pool)
            .await?;
    let applicant_id: (i64,) =
        sqlx::query_as("SELECT id FROM applicants WHERE telegram_user_id = 67890")
            .fetch_one(&pool)
            .await?;

    let result = sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date) \
         VALUES ($1, $2, 77777, 'invalid_status', NOW())"
    )
    .bind(community_id.0)
    .bind(applicant_id.0)
    .execute(&pool)
    .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("chk_join_requests_status"));

    Ok(())
}

#[sqlx::test]
async fn approved_request_allows_new_active_request(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO communities (telegram_chat_id, title, slug) VALUES (-100777, 'C3', 'c3')")
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO applicants (telegram_user_id, first_name) VALUES (11111, 'Carol')")
        .execute(&pool)
        .await?;

    let community_id: (i64,) =
        sqlx::query_as("SELECT id FROM communities WHERE slug = 'c3'")
            .fetch_one(&pool)
            .await?;
    let applicant_id: (i64,) =
        sqlx::query_as("SELECT id FROM applicants WHERE telegram_user_id = 11111")
            .fetch_one(&pool)
            .await?;

    sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date) \
         VALUES ($1, $2, 55555, 'approved', NOW())"
    )
    .bind(community_id.0)
    .bind(applicant_id.0)
    .execute(&pool)
    .await?;

    let result = sqlx::query(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, status, telegram_join_request_date) \
         VALUES ($1, $2, 44444, 'pending_contact', NOW())"
    )
    .bind(community_id.0)
    .bind(applicant_id.0)
    .execute(&pool)
    .await;

    assert!(result.is_ok());

    Ok(())
}
