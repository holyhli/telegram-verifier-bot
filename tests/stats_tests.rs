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
