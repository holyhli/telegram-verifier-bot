#[cfg(test)]
mod tests {
    use verifier_bot::domain::QuestionEventType;

    #[test]
    fn test_question_event_type_variants() {
        let t = QuestionEventType::QuestionPresented;
        assert_eq!(format!("{:?}", t), "QuestionPresented");
        let t2 = QuestionEventType::ValidationFailed;
        assert_eq!(format!("{:?}", t2), "ValidationFailed");
        let t3 = QuestionEventType::AnswerAccepted;
        assert_eq!(format!("{:?}", t3), "AnswerAccepted");
    }

    use verifier_bot::bot::handlers::stats::{StatsCallbackData, StatsPeriod, StatsView};

    #[test]
    fn test_callback_data_roundtrip() {
        // Test all 3 variants round-trip
        let cases = vec![
            StatsCallbackData::SelectCommunity { community_id: 42 },
            StatsCallbackData::SelectPeriod {
                community_id: 42,
                period: StatsPeriod::ThisWeek,
            },
            StatsCallbackData::Navigate {
                community_id: 42,
                period: StatsPeriod::AllTime,
                view: StatsView::Summary,
                page: 3,
            },
        ];
        for case in cases {
            let encoded = case.encode();
            let parsed = StatsCallbackData::parse(&encoded).expect("should parse");
            assert_eq!(case, parsed);
        }
    }

    #[test]
    fn test_callback_data_fits_64_bytes() {
        // Worst case: large community_id, all-time, summary, large page
        let worst = StatsCallbackData::Navigate {
            community_id: 9999999999,
            period: StatsPeriod::AllTime,
            view: StatsView::Summary,
            page: 999,
        };
        let encoded = worst.encode();
        assert!(
            encoded.len() <= 64,
            "callback data exceeds 64 bytes: {} (len={})",
            encoded,
            encoded.len()
        );
    }

    #[test]
    fn test_callback_data_invalid_returns_none() {
        assert!(StatsCallbackData::parse("").is_none());
        assert!(StatsCallbackData::parse("invalid").is_none());
        assert!(StatsCallbackData::parse("sc:").is_none());
        assert!(StatsCallbackData::parse("sc:notanumber").is_none());
    }
}

// --- QuestionEventRepo DB Integration Tests ---

use sqlx::PgPool;
use verifier_bot::db::QuestionEventRepo;
use verifier_bot::domain::QuestionEventType;

async fn seed_community(pool: &PgPool) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO communities (telegram_chat_id, title, slug)
         VALUES (-1009888888888, 'Stats Test Community', 'stats-test')
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("seed community");
    id
}

async fn seed_question(pool: &PgPool, community_id: i64, position: i32) -> i64 {
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

async fn seed_join_request(pool: &PgPool, community_id: i64, applicant_id: i64) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, telegram_join_request_date, status)
         VALUES ($1, $2, 100000, NOW(), 'pending_contact')
         RETURNING id",
    )
    .bind(community_id)
    .bind(applicant_id)
    .fetch_one(pool)
    .await
    .expect("seed join_request");
    id
}

#[sqlx::test(migrations = "./migrations")]
async fn test_question_event_repo_create(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let question_id = seed_question(&pool, community_id, 1).await;
    let applicant_id = seed_applicant(&pool, 700001).await;
    let jr_id = seed_join_request(&pool, community_id, applicant_id).await;

    let event = QuestionEventRepo::create(
        &pool,
        jr_id,
        question_id,
        applicant_id,
        QuestionEventType::QuestionPresented,
        Some(serde_json::json!({"position": 1})),
    )
    .await
    .expect("create event");

    assert_eq!(event.join_request_id, jr_id);
    assert_eq!(event.community_question_id, question_id);
    assert_eq!(event.applicant_id, applicant_id);
    assert_eq!(event.event_type, QuestionEventType::QuestionPresented);
    assert!(event.metadata.is_some());
    assert!(event.id > 0);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_find_by_join_request_id(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let q1 = seed_question(&pool, community_id, 1).await;
    let q2 = seed_question(&pool, community_id, 2).await;
    let applicant_id = seed_applicant(&pool, 700002).await;
    let jr_id = seed_join_request(&pool, community_id, applicant_id).await;

    // Create 3 events for the same join_request
    QuestionEventRepo::create(&pool, jr_id, q1, applicant_id, QuestionEventType::QuestionPresented, None)
        .await.expect("event 1");
    QuestionEventRepo::create(&pool, jr_id, q1, applicant_id, QuestionEventType::AnswerAccepted, None)
        .await.expect("event 2");
    QuestionEventRepo::create(&pool, jr_id, q2, applicant_id, QuestionEventType::QuestionPresented, None)
        .await.expect("event 3");

    let events = QuestionEventRepo::find_by_join_request_id(&pool, jr_id)
        .await
        .expect("find events");

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, QuestionEventType::QuestionPresented);
    assert_eq!(events[1].event_type, QuestionEventType::AnswerAccepted);
    assert_eq!(events[2].event_type, QuestionEventType::QuestionPresented);

    // Verify empty result for non-existent join_request
    let empty = QuestionEventRepo::find_by_join_request_id(&pool, 999999)
        .await
        .expect("find empty");
    assert!(empty.is_empty());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_count_validation_failures(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let q1 = seed_question(&pool, community_id, 1).await;
    let q2 = seed_question(&pool, community_id, 2).await;
    let applicant_id = seed_applicant(&pool, 700003).await;
    let jr_id = seed_join_request(&pool, community_id, applicant_id).await;

    // 3 validation_failed for q1
    for _ in 0..3 {
        QuestionEventRepo::create(&pool, jr_id, q1, applicant_id, QuestionEventType::ValidationFailed, None)
            .await.expect("vf q1");
    }
    // 1 validation_failed for q2
    QuestionEventRepo::create(&pool, jr_id, q2, applicant_id, QuestionEventType::ValidationFailed, None)
        .await.expect("vf q2");
    // 1 answer_accepted for q1 (should NOT count)
    QuestionEventRepo::create(&pool, jr_id, q1, applicant_id, QuestionEventType::AnswerAccepted, None)
        .await.expect("aa q1");

    let mut counts = QuestionEventRepo::count_validation_failures(&pool, jr_id)
        .await
        .expect("count failures");

    // Sort by community_question_id for deterministic assertion
    counts.sort_by_key(|(qid, _)| *qid);

    assert_eq!(counts.len(), 2);
    assert_eq!(counts[0], (q1, 3));
    assert_eq!(counts[1], (q2, 1));

    Ok(())
}

// --- StatsFormatter Integration Tests ---

#[cfg(test)]
mod formatter_tests {
    use verifier_bot::bot::handlers::stats::{StatsCallbackData, StatsPeriod, StatsView};
    use verifier_bot::services::stats_formatter::{
        ActiveApplicantInfo, ApplicantSummary, QuestionTiming, StatsFormatter,
    };

    #[test]
    fn test_format_community_selection() {
        let communities = vec![
            (1_i64, "DeFi Amsterdam".to_string()),
            (2_i64, "Rust Developers".to_string()),
        ];
        let (text, keyboard) = StatsFormatter::format_community_selection(&communities);

        // Verify text content
        assert!(text.contains("📊"));
        assert!(text.contains("<b>Stats</b>"));
        assert!(text.contains("Select a community"));

        // Verify keyboard: 2 rows (one per community), each with 1 button
        assert_eq!(keyboard.len(), 2);
        assert_eq!(keyboard[0].len(), 1);
        assert_eq!(keyboard[1].len(), 1);

        // Verify labels
        assert_eq!(keyboard[0][0].0, "DeFi Amsterdam");
        assert_eq!(keyboard[1][0].0, "Rust Developers");

        // Verify callback data parses correctly
        let cb1 = StatsCallbackData::parse(&keyboard[0][0].1).unwrap();
        assert_eq!(cb1, StatsCallbackData::SelectCommunity { community_id: 1 });
        let cb2 = StatsCallbackData::parse(&keyboard[1][0].1).unwrap();
        assert_eq!(cb2, StatsCallbackData::SelectCommunity { community_id: 2 });
    }

    #[test]
    fn test_format_period_selection() {
        let (text, keyboard) = StatsFormatter::format_period_selection("DeFi Amsterdam", 42);

        // Verify text
        assert!(text.contains("<b>DeFi Amsterdam</b>"));
        assert!(text.contains("Select time period"));

        // Verify 2x2 grid
        assert_eq!(keyboard.len(), 2);
        assert_eq!(keyboard[0].len(), 2);
        assert_eq!(keyboard[1].len(), 2);

        // Verify 4 period buttons with correct callbacks
        let expected = [
            ("Today", "sp:42:t"),
            ("This Week", "sp:42:w"),
            ("This Month", "sp:42:m"),
            ("All Time", "sp:42:a"),
        ];
        let flat: Vec<&(String, String)> = keyboard.iter().flat_map(|r| r.iter()).collect();
        for (i, (label, cb)) in expected.iter().enumerate() {
            assert_eq!(&flat[i].0, label);
            assert_eq!(&flat[i].1, cb);
        }
    }

    #[test]
    fn test_format_active_view_pagination() {
        // Create 25 applicants
        let applicants: Vec<ActiveApplicantInfo> = (1..=25)
            .map(|i| ActiveApplicantInfo {
                join_request_id: i as i64,
                applicant_id: i as i64,
                name: Some(format!("User{}", i)),
                username: Some(format!("user{}", i)),
                current_question_position: 3,
                total_questions: 5,
                current_question_text: "How did you hear about us?".to_string(),
                time_on_current_secs: 1380,
                time_started_secs: 2700,
            })
            .collect();

        let total_pages = 3; // 25 / 10 = 3 pages
        let (text, keyboard) =
            StatsFormatter::format_active_view("DeFi", 42, &StatsPeriod::Today, &applicants, 1, total_pages);

        // Verify header
        assert!(text.contains("<b>DeFi</b>"));
        assert!(text.contains("Active (Today)"));
        assert!(text.contains("25 applicants"));

        // Page 1 should show items 1-10
        assert!(text.contains("1. User1 (@user1)"));
        assert!(text.contains("10. User10 (@user10)"));
        // Should NOT show item 11
        assert!(!text.contains("11. User11"));

        // Keyboard: 1 nav row, no Prev (page 1), has Next
        assert_eq!(keyboard.len(), 1);
        let nav = &keyboard[0];
        // No Prev on page 1
        assert!(!nav.iter().any(|(label, _)| label == "◀ Prev"));
        // Has toggle and Next
        assert!(nav.iter().any(|(label, _)| label == "Active | Summary"));
        assert!(nav.iter().any(|(label, _)| label == "Next ▶"));

        // Verify Next callback goes to page 2
        let next_btn = nav.iter().find(|(label, _)| label == "Next ▶").unwrap();
        let next_cb = StatsCallbackData::parse(&next_btn.1).unwrap();
        assert_eq!(
            next_cb,
            StatsCallbackData::Navigate {
                community_id: 42,
                period: StatsPeriod::Today,
                view: StatsView::Active,
                page: 2,
            }
        );
    }

    #[test]
    fn test_format_active_view_middle_page() {
        let applicants: Vec<ActiveApplicantInfo> = (1..=25)
            .map(|i| ActiveApplicantInfo {
                join_request_id: i as i64,
                applicant_id: i as i64,
                name: Some(format!("User{}", i)),
                username: None,
                current_question_position: 1,
                total_questions: 3,
                current_question_text: "Name?".to_string(),
                time_on_current_secs: 60,
                time_started_secs: 120,
            })
            .collect();

        let (text, keyboard) =
            StatsFormatter::format_active_view("Test", 1, &StatsPeriod::AllTime, &applicants, 2, 3);

        // Page 2 should show items 11-20
        assert!(text.contains("11. User11"));
        assert!(text.contains("20. User20"));

        // Keyboard should have Prev and Next
        let nav = &keyboard[0];
        assert!(nav.iter().any(|(l, _)| l == "◀ Prev"));
        assert!(nav.iter().any(|(l, _)| l == "Next ▶"));
    }

    #[test]
    fn test_format_summary_view_with_warning() {
        let summaries = vec![ApplicantSummary {
            join_request_id: 1,
            applicant_id: 1,
            name: Some("John".to_string()),
            username: Some("johndoe".to_string()),
            status: "approved".to_string(),
            question_timings: vec![
                QuestionTiming {
                    community_question_id: 1,
                    position: 1,
                    question_text: "Name".to_string(),
                    duration_secs: Some(72),
                    retry_count: 0,
                },
                QuestionTiming {
                    community_question_id: 2,
                    position: 2,
                    question_text: "Occupation".to_string(),
                    duration_secs: Some(930),  // 15m 30s — >10m triggers ⚠️
                    retry_count: 2,
                },
                QuestionTiming {
                    community_question_id: 3,
                    position: 3,
                    question_text: "Referral".to_string(),
                    duration_secs: Some(45),
                    retry_count: 0,
                },
            ],
            total_time_secs: Some(1047),
            total_retries: 2,
        }];

        let (text, keyboard) = StatsFormatter::format_summary_view(
            "DeFi", 42, &StatsPeriod::ThisWeek, &summaries, 1, 1,
        );

        // Verify header
        assert!(text.contains("Summary (This Week)"));

        // Verify applicant line
        assert!(text.contains("John (@johndoe)"));
        assert!(text.contains("✅ Approved"));

        // Verify question timings
        assert!(text.contains("Q1 (Name): 1m 12s"));
        assert!(text.contains("Q2 (Occupation): 15m 30s ⚠️"));
        assert!(text.contains("Q3 (Referral): 45s"));

        // Verify totals
        assert!(text.contains("Retries: 2"));

        // Single page: no Prev, no Next
        let nav = &keyboard[0];
        assert!(!nav.iter().any(|(l, _)| l == "◀ Prev"));
        assert!(!nav.iter().any(|(l, _)| l == "Next ▶"));
        // But toggle is there
        assert!(nav.iter().any(|(l, _)| l == "Active | Summary"));
    }

    #[test]
    fn test_format_message_within_limit() {
        // Create many applicants with long names to stress the 4096 char limit
        let applicants: Vec<ActiveApplicantInfo> = (1..=100)
            .map(|i| ActiveApplicantInfo {
                join_request_id: i as i64,
                applicant_id: i as i64,
                name: Some(format!("VeryLongUserName_{}_WithExtraText", i)),
                username: Some(format!("username_{}_with_a_long_handle", i)),
                current_question_position: 5,
                total_questions: 10,
                current_question_text: "This is a fairly long question text that simulates a real questionnaire field".to_string(),
                time_on_current_secs: 7200,
                time_started_secs: 86400,
            })
            .collect();

        let (text, _) =
            StatsFormatter::format_active_view("Community", 1, &StatsPeriod::AllTime, &applicants, 1, 10);

        assert!(
            text.len() <= 4096,
            "message exceeds 4096 chars: len={}",
            text.len()
        );
    }

    #[test]
    fn test_format_community_selection_html_escape() {
        let communities = vec![(1_i64, "C++ & Rust <Advanced>".to_string())];
        let (text, _) = StatsFormatter::format_community_selection(&communities);
        // HTML entities should NOT appear in the header (community name is only in keyboard, not text)
        assert!(!text.contains("<Advanced>"));
    }

    #[test]
    fn test_format_empty_active_view() {
        let (text, _) = StatsFormatter::format_active_view(
            "Test", 1, &StatsPeriod::Today, &[], 1, 1,
        );
        assert!(text.contains("No active applicants"));
    }

    #[test]
    fn test_format_empty_summary_view() {
        let (text, _) = StatsFormatter::format_summary_view(
            "Test", 1, &StatsPeriod::Today, &[], 1, 1,
        );
        assert!(text.contains("No applicants"));
    }
}

// --- StatsService Tests ---

use chrono::{TimeZone, Utc};
use verifier_bot::domain::QuestionEvent;
use verifier_bot::services::StatsService;

async fn activate_join_request(pool: &PgPool, jr_id: i64) {
    sqlx::query("UPDATE join_requests SET status = 'questionnaire_in_progress' WHERE id = $1")
        .bind(jr_id)
        .execute(pool)
        .await
        .expect("activate join request");
}

async fn submit_join_request(pool: &PgPool, jr_id: i64) {
    sqlx::query("UPDATE join_requests SET status = 'submitted' WHERE id = $1")
        .bind(jr_id)
        .execute(pool)
        .await
        .expect("submit join request");
}

async fn seed_session(pool: &PgPool, jr_id: i64, position: i32, state: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO applicant_sessions (join_request_id, current_question_position, state) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(jr_id)
    .bind(position)
    .bind(state)
    .fetch_one(pool)
    .await
    .expect("seed session");
    id
}

async fn seed_question_event(pool: &PgPool, jr_id: i64, question_id: i64, applicant_id: i64, event_type: &str) {
    sqlx::query(
        "INSERT INTO question_events (join_request_id, community_question_id, applicant_id, event_type) VALUES ($1, $2, $3, $4)",
    )
    .bind(jr_id)
    .bind(question_id)
    .bind(applicant_id)
    .bind(event_type)
    .execute(pool)
    .await
    .expect("seed question event");
}

async fn seed_join_request_at(
    pool: &PgPool,
    community_id: i64,
    applicant_id: i64,
    created_at: chrono::DateTime<Utc>,
    status: &str,
) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO join_requests (community_id, applicant_id, telegram_user_chat_id, telegram_join_request_date, status, created_at) VALUES ($1, $2, 100000, NOW(), $3, $4) RETURNING id",
    )
    .bind(community_id)
    .bind(applicant_id)
    .bind(status)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .expect("seed join_request_at");
    id
}

#[sqlx::test(migrations = "./migrations")]
async fn test_get_active_applicants(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let q1 = seed_question(&pool, community_id, 1).await;
    let _q2 = seed_question(&pool, community_id, 2).await;
    let _q3 = seed_question(&pool, community_id, 3).await;

    let app1 = seed_applicant(&pool, 800001).await;
    let jr1 = seed_join_request(&pool, community_id, app1).await;
    activate_join_request(&pool, jr1).await;
    seed_session(&pool, jr1, 1, "awaiting_answer").await;
    seed_question_event(&pool, jr1, q1, app1, "question_presented").await;

    let app2 = seed_applicant(&pool, 800002).await;
    let jr2 = seed_join_request(&pool, community_id, app2).await;
    activate_join_request(&pool, jr2).await;
    seed_session(&pool, jr2, 1, "awaiting_answer").await;
    seed_question_event(&pool, jr2, q1, app2, "question_presented").await;

    let app3 = seed_applicant(&pool, 800003).await;
    let jr3 = seed_join_request(&pool, community_id, app3).await;
    submit_join_request(&pool, jr3).await;
    seed_session(&pool, jr3, 3, "completed").await;

    let result = StatsService::get_active_applicants(&pool, community_id)
        .await
        .expect("get_active_applicants");

    assert_eq!(result.len(), 2);
    let ids: Vec<i64> = result.iter().map(|r| r.join_request_id).collect();
    assert!(ids.contains(&jr1));
    assert!(ids.contains(&jr2));
    assert!(!ids.contains(&jr3));

    assert_eq!(result[0].total_questions, 3);
    assert_eq!(result[0].current_question_position, 1);
    assert!(result[0].time_started_secs >= 0);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_get_period_summary_filters_by_time(pool: PgPool) -> sqlx::Result<()> {
    let community_id = seed_community(&pool).await;
    let _q1 = seed_question(&pool, community_id, 1).await;

    let app1 = seed_applicant(&pool, 810001).await;
    let app2 = seed_applicant(&pool, 810002).await;
    let app3 = seed_applicant(&pool, 810003).await;

    let two_days_ago = Utc::now() - chrono::Duration::days(2);
    let one_day_ago = Utc::now() - chrono::Duration::days(1);
    let now = Utc::now();

    seed_join_request_at(&pool, community_id, app1, two_days_ago, "submitted").await;
    seed_join_request_at(&pool, community_id, app2, one_day_ago, "questionnaire_in_progress").await;
    seed_join_request_at(&pool, community_id, app3, now, "questionnaire_in_progress").await;

    let period_start = Utc::now() - chrono::Duration::hours(25);
    let result = StatsService::get_period_summary(&pool, community_id, period_start)
        .await
        .expect("get_period_summary");

    assert_eq!(result.len(), 2);
    let applicant_ids: Vec<i64> = result.iter().map(|r| r.applicant_id).collect();
    assert!(applicant_ids.contains(&app2));
    assert!(applicant_ids.contains(&app3));
    assert!(!applicant_ids.contains(&app1));

    Ok(())
}

#[test]
fn test_compute_per_question_timing() {
    let base = Utc.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap();

    let events = vec![
        QuestionEvent {
            id: 1,
            join_request_id: 1,
            community_question_id: 100,
            applicant_id: 1,
            event_type: QuestionEventType::QuestionPresented,
            metadata: None,
            created_at: base,
        },
        QuestionEvent {
            id: 2,
            join_request_id: 1,
            community_question_id: 100,
            applicant_id: 1,
            event_type: QuestionEventType::ValidationFailed,
            metadata: None,
            created_at: base + chrono::Duration::seconds(30),
        },
        QuestionEvent {
            id: 3,
            join_request_id: 1,
            community_question_id: 100,
            applicant_id: 1,
            event_type: QuestionEventType::ValidationFailed,
            metadata: None,
            created_at: base + chrono::Duration::seconds(60),
        },
        QuestionEvent {
            id: 4,
            join_request_id: 1,
            community_question_id: 100,
            applicant_id: 1,
            event_type: QuestionEventType::AnswerAccepted,
            metadata: None,
            created_at: base + chrono::Duration::seconds(90),
        },
        QuestionEvent {
            id: 5,
            join_request_id: 1,
            community_question_id: 200,
            applicant_id: 1,
            event_type: QuestionEventType::QuestionPresented,
            metadata: None,
            created_at: base + chrono::Duration::seconds(100),
        },
        QuestionEvent {
            id: 6,
            join_request_id: 1,
            community_question_id: 200,
            applicant_id: 1,
            event_type: QuestionEventType::AnswerAccepted,
            metadata: None,
            created_at: base + chrono::Duration::seconds(145),
        },
    ];

    let timings = StatsService::compute_per_question_timing(&events);

    assert_eq!(timings.len(), 2);

    assert_eq!(timings[0].community_question_id, 100);
    assert_eq!(timings[0].duration_secs, Some(90));
    assert_eq!(timings[0].retry_count, 2);

    assert_eq!(timings[1].community_question_id, 200);
    assert_eq!(timings[1].duration_secs, Some(45));
    assert_eq!(timings[1].retry_count, 0);
}

// --- Stats Command & Callback Handler Tests ---

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use teloxide::RequestError;
use teloxide::types::InlineKeyboardMarkup;
use verifier_bot::bot::handlers::TelegramApi;
use verifier_bot::bot::handlers::stats::{StatsCallbackData, StatsCommandInput, StatsPeriod, process_stats_command};
use verifier_bot::config::{Config, BotSettings};

#[derive(Debug, Clone)]
struct FakeTelegramApi {
    sent_messages: Arc<Mutex<Vec<(i64, String)>>>,
    keyboards_sent: Arc<Mutex<Vec<(i64, String, Vec<Vec<(String, String)>>)>>>,
    sent_html_messages: Arc<Mutex<Vec<(i64, String, Option<InlineKeyboardMarkup>)>>>,
    edited_messages_with_markup: Arc<Mutex<Vec<(i64, i32, String, Option<Vec<Vec<(String, String)>>>)>>>,
    cleared_markup: Arc<Mutex<Vec<(i64, i64)>>>,
    answered_callbacks: Arc<Mutex<Vec<(String, String)>>>,
    approved_requests: Arc<Mutex<Vec<(i64, i64)>>>,
    declined_requests: Arc<Mutex<Vec<(i64, i64)>>>,
}

impl FakeTelegramApi {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            keyboards_sent: Arc::new(Mutex::new(Vec::new())),
            sent_html_messages: Arc::new(Mutex::new(Vec::new())),
            edited_messages_with_markup: Arc::new(Mutex::new(vec![])),
            cleared_markup: Arc::new(Mutex::new(Vec::new())),
            answered_callbacks: Arc::new(Mutex::new(Vec::new())),
            approved_requests: Arc::new(Mutex::new(Vec::new())),
            declined_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn keyboards_sent(&self) -> Vec<(i64, String, Vec<Vec<(String, String)>>)> {
        self.keyboards_sent
            .lock()
            .expect("lock keyboards_sent")
            .clone()
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
        self.sent_messages.lock().unwrap().push((chat_id, text));
        Ok(())
    }

    async fn send_message_html(
        &self,
        chat_id: i64,
        text: String,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<i64, RequestError> {
        let mut msgs = self.sent_html_messages.lock().unwrap();
        msgs.push((chat_id, text, reply_markup));
        Ok(msgs.len() as i64)
    }

    async fn edit_message_html(
        &self,
        _chat_id: i64,
        _message_id: i64,
        _text: String,
    ) -> Result<(), RequestError> {
        Ok(())
    }

    async fn edit_message_html_with_markup(
        &self,
        chat_id: i64,
        message_id: i32,
        text: String,
        reply_markup: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<(), RequestError> {
        self.edited_messages_with_markup.lock().unwrap().push((chat_id, message_id, text, reply_markup));
        Ok(())
    }

    async fn clear_message_reply_markup(
        &self,
        chat_id: i64,
        message_id: i64,
    ) -> Result<(), RequestError> {
        self.cleared_markup.lock().unwrap().push((chat_id, message_id));
        Ok(())
    }

    async fn answer_callback_query(
        &self,
        callback_query_id: String,
        text: String,
    ) -> Result<(), RequestError> {
        self.answered_callbacks.lock().unwrap().push((callback_query_id, text));
        Ok(())
    }

    async fn approve_chat_join_request(
        &self,
        chat_id: i64,
        user_id: i64,
    ) -> Result<(), RequestError> {
        self.approved_requests.lock().unwrap().push((chat_id, user_id));
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

    async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: String,
        keyboard: Vec<Vec<(String, String)>>,
    ) -> Result<(), RequestError> {
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

fn make_test_config(moderator_ids: Vec<i64>) -> Config {
    Config {
        bot_token: "test".to_string(),
        database_url: "test".to_string(),
        default_moderator_chat_id: -100999,
        allowed_moderator_ids: moderator_ids,
        use_webhooks: false,
        public_webhook_url: None,
        server_port: 8080,
        rust_log: "info".to_string(),
        bot_settings: BotSettings::default(),
        communities: vec![],
    }
}

async fn seed_community_with_title(pool: &PgPool, chat_id: i64, title: &str, slug: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO communities (telegram_chat_id, title, slug)
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(chat_id)
    .bind(title)
    .bind(slug)
    .fetch_one(pool)
    .await
    .expect("seed community with title");
    id
}

#[sqlx::test(migrations = "./migrations")]
async fn test_stats_command_multi_community(pool: PgPool) -> sqlx::Result<()> {
    let id1 = seed_community_with_title(&pool, -1009111111111, "Alpha Community", "alpha").await;
    let id2 = seed_community_with_title(&pool, -1009222222222, "Beta Community", "beta").await;

    let config = make_test_config(vec![12345]);
    let api = FakeTelegramApi::new();

    let input = StatsCommandInput {
        chat_id: 99999,
        telegram_user_id: 12345,
    };

    process_stats_command(&api, &pool, &config, input)
        .await
        .expect("process_stats_command");

    let keyboards = api.keyboards_sent();
    assert_eq!(keyboards.len(), 1, "should send exactly one keyboard message");

    let (chat_id, text, keyboard) = &keyboards[0];
    assert_eq!(*chat_id, 99999);
    assert!(text.contains("Select a community"));

    // Should have 2 rows (one per community)
    assert_eq!(keyboard.len(), 2);
    assert_eq!(keyboard[0][0].0, "Alpha Community");
    assert_eq!(keyboard[1][0].0, "Beta Community");

    // Verify callback data encodes correctly
    let cb1 = StatsCallbackData::parse(&keyboard[0][0].1).unwrap();
    assert_eq!(cb1, StatsCallbackData::SelectCommunity { community_id: id1 });
    let cb2 = StatsCallbackData::parse(&keyboard[1][0].1).unwrap();
    assert_eq!(cb2, StatsCallbackData::SelectCommunity { community_id: id2 });

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_stats_command_single_community(pool: PgPool) -> sqlx::Result<()> {
    let id = seed_community_with_title(&pool, -1009333333333, "Solo Community", "solo").await;

    let config = make_test_config(vec![12345]);
    let api = FakeTelegramApi::new();

    let input = StatsCommandInput {
        chat_id: 88888,
        telegram_user_id: 12345,
    };

    process_stats_command(&api, &pool, &config, input)
        .await
        .expect("process_stats_command");

    let keyboards = api.keyboards_sent();
    assert_eq!(keyboards.len(), 1, "should send exactly one keyboard message");

    let (chat_id, text, keyboard) = &keyboards[0];
    assert_eq!(*chat_id, 88888);
    assert!(text.contains("Solo Community"));
    assert!(text.contains("Select time period"));

    // Should have 2x2 grid (4 period buttons)
    assert_eq!(keyboard.len(), 2);
    assert_eq!(keyboard[0].len(), 2);
    assert_eq!(keyboard[1].len(), 2);

    // Verify period buttons
    assert_eq!(keyboard[0][0].0, "Today");
    assert_eq!(keyboard[0][1].0, "This Week");
    assert_eq!(keyboard[1][0].0, "This Month");
    assert_eq!(keyboard[1][1].0, "All Time");

    // Verify callback data for first period button
    let cb = StatsCallbackData::parse(&keyboard[0][0].1).unwrap();
    assert_eq!(
        cb,
        StatsCallbackData::SelectPeriod {
            community_id: id,
            period: StatsPeriod::Today,
        }
    );

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_stats_command_unauthorized(pool: PgPool) -> sqlx::Result<()> {
    seed_community_with_title(&pool, -1009444444444, "Test Community", "test-unauth").await;

    let config = make_test_config(vec![12345]);
    let api = FakeTelegramApi::new();

    let input = StatsCommandInput {
        chat_id: 77777,
        telegram_user_id: 99999, // NOT in allowed list
    };

    process_stats_command(&api, &pool, &config, input)
        .await
        .expect("process_stats_command");

    // Should NOT send any messages
    let messages = api.sent_messages();
    assert!(messages.is_empty(), "unauthorized user should get no messages");

    let keyboards = api.keyboards_sent();
    assert!(keyboards.is_empty(), "unauthorized user should get no keyboards");

    Ok(())
}

// --- Stats Callback Handler Tests ---

mod callback_handler_tests {
    use sqlx::PgPool;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use teloxide::RequestError;
    use teloxide::types::InlineKeyboardMarkup;

    use verifier_bot::bot::handlers::TelegramApi;
    use verifier_bot::bot::handlers::stats::{process_stats_callback, StatsCallbackInput};
    use verifier_bot::config::{BotSettings, Config};

    use super::{seed_community, seed_question};

    #[derive(Debug, Clone)]
    struct FakeCallbackApi {
        answered_callbacks: Arc<Mutex<Vec<(String, String)>>>,
        edited_messages_with_markup: Arc<Mutex<Vec<(i64, i32, String, Option<Vec<Vec<(String, String)>>>)>>>,
    }

    impl FakeCallbackApi {
        fn new() -> Self {
            Self {
                answered_callbacks: Arc::new(Mutex::new(Vec::new())),
                edited_messages_with_markup: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl TelegramApi for FakeCallbackApi {
        async fn send_message(&self, _chat_id: i64, _text: String) -> Result<(), RequestError> {
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

        async fn edit_message_html_with_markup(
            &self,
            chat_id: i64,
            message_id: i32,
            text: String,
            reply_markup: Option<Vec<Vec<(String, String)>>>,
        ) -> Result<(), RequestError> {
            self.edited_messages_with_markup.lock().unwrap().push((chat_id, message_id, text, reply_markup));
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
            callback_query_id: String,
            text: String,
        ) -> Result<(), RequestError> {
            self.answered_callbacks.lock().unwrap().push((callback_query_id, text));
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
            _chat_id: i64,
            _user_id: i64,
        ) -> Result<(), RequestError> {
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

    fn cb_test_config(allowed_moderator_ids: Vec<i64>) -> Config {
        Config {
            bot_token: "token".to_string(),
            database_url: "postgres://example".to_string(),
            default_moderator_chat_id: -100_999_000_111,
            allowed_moderator_ids,
            use_webhooks: false,
            public_webhook_url: None,
            server_port: 8080,
            rust_log: "info".to_string(),
            bot_settings: BotSettings::default(),
            communities: vec![],
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_stats_callback_community_selection(pool: PgPool) -> sqlx::Result<()> {
        let community_id = seed_community(&pool).await;
        let api = FakeCallbackApi::new();
        let config = cb_test_config(vec![9001]);

        let input = StatsCallbackInput {
            chat_id: -100_999_000_111,
            message_id: 42,
            callback_query_id: "cb-stats-1".to_string(),
            telegram_user_id: 9001,
            data: format!("sc:{}", community_id),
        };

        process_stats_callback(&api, &pool, &config, input)
            .await
            .expect("stats callback should succeed");

        // Verify callback was answered
        let callbacks = api.answered_callbacks.lock().unwrap().clone();
        assert_eq!(callbacks.len(), 1);
        assert_eq!(callbacks[0].0, "cb-stats-1");

        // Verify message was edited with period selection keyboard
        let edits = api.edited_messages_with_markup.lock().unwrap().clone();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].0, -100_999_000_111);
        assert_eq!(edits[0].1, 42);
        assert!(edits[0].2.contains("Select time period"));
        assert!(edits[0].2.contains("Stats Test Community"));

        // Verify keyboard has period buttons
        let keyboard = edits[0].3.as_ref().expect("keyboard present");
        let flat: Vec<&(String, String)> = keyboard.iter().flat_map(|r| r.iter()).collect();
        assert_eq!(flat.len(), 4); // Today, This Week, This Month, All Time
        assert!(flat.iter().any(|(label, _)| label == "Today"));
        assert!(flat.iter().any(|(label, _)| label == "All Time"));

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_stats_callback_period_selection(pool: PgPool) -> sqlx::Result<()> {
        let community_id = seed_community(&pool).await;
        let _q1 = seed_question(&pool, community_id, 1).await;
        let api = FakeCallbackApi::new();
        let config = cb_test_config(vec![9002]);

        let input = StatsCallbackInput {
            chat_id: -100_999_000_111,
            message_id: 43,
            callback_query_id: "cb-stats-2".to_string(),
            telegram_user_id: 9002,
            data: format!("sp:{}:w", community_id),
        };

        process_stats_callback(&api, &pool, &config, input)
            .await
            .expect("stats period callback should succeed");

        // Verify message was edited with active view
        let edits = api.edited_messages_with_markup.lock().unwrap().clone();
        assert_eq!(edits.len(), 1);
        assert!(edits[0].2.contains("Active (This Week)"));
        assert!(edits[0].2.contains("Stats Test Community"));

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_stats_callback_navigation(pool: PgPool) -> sqlx::Result<()> {
        let community_id = seed_community(&pool).await;
        let _q1 = seed_question(&pool, community_id, 1).await;
        let api = FakeCallbackApi::new();
        let config = cb_test_config(vec![9003]);

        // Navigate to summary view
        let input = StatsCallbackInput {
            chat_id: -100_999_000_111,
            message_id: 44,
            callback_query_id: "cb-stats-3".to_string(),
            telegram_user_id: 9003,
            data: format!("sn:{}:w:s:1", community_id),
        };

        process_stats_callback(&api, &pool, &config, input)
            .await
            .expect("stats navigation callback should succeed");

        // Verify summary view shown
        let edits = api.edited_messages_with_markup.lock().unwrap().clone();
        assert_eq!(edits.len(), 1);
        assert!(edits[0].2.contains("Summary (This Week)"));
        assert!(edits[0].2.contains("Stats Test Community"));

        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_stats_callback_unauthorized(pool: PgPool) -> sqlx::Result<()> {
        let community_id = seed_community(&pool).await;
        let api = FakeCallbackApi::new();
        // Config with moderator 9004 -- caller will be 7777 (not authorized)
        let config = cb_test_config(vec![9004]);

        let input = StatsCallbackInput {
            chat_id: -100_999_000_111,
            message_id: 45,
            callback_query_id: "cb-stats-unauth".to_string(),
            telegram_user_id: 7777,
            data: format!("sc:{}", community_id),
        };

        process_stats_callback(&api, &pool, &config, input)
            .await
            .expect("unauthorized callback should succeed without error");

        // Callback should still be answered (silently)
        let callbacks = api.answered_callbacks.lock().unwrap().clone();
        assert_eq!(callbacks.len(), 1);

        // No message edit should have happened
        let edits = api.edited_messages_with_markup.lock().unwrap().clone();
        assert_eq!(edits.len(), 0);

        Ok(())
    }
}
