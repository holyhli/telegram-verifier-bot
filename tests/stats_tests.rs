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
