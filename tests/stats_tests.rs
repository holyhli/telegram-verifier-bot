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
            name: Some("John".to_string()),
            username: Some("johndoe".to_string()),
            status: "approved".to_string(),
            question_timings: vec![
                QuestionTiming {
                    position: 1,
                    question_text: "Name".to_string(),
                    duration_secs: Some(72),
                    retry_count: 0,
                },
                QuestionTiming {
                    position: 2,
                    question_text: "Occupation".to_string(),
                    duration_secs: Some(930),  // 15m 30s — >10m triggers ⚠️
                    retry_count: 2,
                },
                QuestionTiming {
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
