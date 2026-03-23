#[cfg(test)]
mod tests {
    use verifier_bot::domain::{QuestionEvent, QuestionEventType};

    #[test]
    fn test_question_event_type_variants() {
        let t = QuestionEventType::QuestionPresented;
        assert_eq!(format!("{:?}", t), "QuestionPresented");
        let t2 = QuestionEventType::ValidationFailed;
        assert_eq!(format!("{:?}", t2), "ValidationFailed");
        let t3 = QuestionEventType::AnswerAccepted;
        assert_eq!(format!("{:?}", t3), "AnswerAccepted");
    }
}
