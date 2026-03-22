use crate::domain::Language;

/// Provides bilingual message templates for user-facing bot messages.
/// All messages support both English and Ukrainian languages.
pub struct Messages;

impl Messages {
    /// Returns the welcome message with instructions to answer questions.
    ///
    /// # Arguments
    /// * `first_name` - User's first name
    /// * `community_title` - Name of the community they're joining
    /// * `lang` - Selected language
    ///
    /// # Example
    /// ```ignore
    /// let msg = Messages::welcome_message("Alice", "Rust Developers", Language::English);
    /// assert!(msg.contains("Hi Alice!"));
    /// ```
    pub fn welcome_message(first_name: &str, community_title: &str, lang: Language) -> String {
        match lang {
            Language::English => format!(
                "Hi {}! I saw your request to join {}.\n\nBefore a moderator reviews it, please answer a few quick questions.",
                first_name, community_title
            ),
            Language::Ukrainian => format!(
                "Привіт, {}! Я бачу твій запит приєднатися до {}.\n\nПеред тим як модератор його розгляне, будь ласка, дай відповідь на кілька швидких питань.",
                first_name, community_title
            ),
        }
    }

    /// Returns the language selection message (bilingual).
    /// This message is shown in both languages to help users choose their preferred language.
    ///
    /// # Arguments
    /// * `first_name` - User's first name
    /// * `community_title` - Name of the community they're joining
    ///
    /// # Example
    /// ```ignore
    /// let msg = Messages::language_selection_message("Alice", "Rust Developers");
    /// assert!(msg.contains("English"));
    /// assert!(msg.contains("Ukrainian"));
    /// ```
    pub fn language_selection_message(first_name: &str, community_title: &str) -> String {
        format!(
            "Hi {}! I saw your request to join {}.\n\nPlease select your preferred language:\n\n🇬🇧 English\n🇺🇦 Українська",
            first_name, community_title
        )
    }

    /// Returns the completion message confirming the application was submitted.
    ///
    /// # Arguments
    /// * `lang` - Selected language
    ///
    /// # Example
    /// ```ignore
    /// let msg = Messages::completion_message(Language::English);
    /// assert!(msg.contains("submitted"));
    /// ```
    pub fn completion_message(lang: Language) -> String {
        match lang {
            Language::English => "Thanks — your application has been submitted to the moderators.\nYou'll be notified once a decision is made.".to_string(),
            Language::Ukrainian => "Дякую! Твою заявку відправлено модераторам.\nТи отримаєш повідомлення, коли буде прийнято рішення.".to_string(),
        }
    }

    /// Returns the error message for required fields that are missing.
    ///
    /// # Arguments
    /// * `lang` - Selected language
    ///
    /// # Example
    /// ```ignore
    /// let msg = Messages::required_field_error(Language::English);
    /// assert!(msg.contains("required"));
    /// ```
    pub fn required_field_error(lang: Language) -> String {
        match lang {
            Language::English => "This field is required. Please provide an answer.".to_string(),
            Language::Ukrainian => "Це поле обов'язкове. Будь ласка, дай відповідь.".to_string(),
        }
    }

    /// Returns the error message for answers that lack sufficient detail.
    ///
    /// # Arguments
    /// * `lang` - Selected language
    ///
    /// # Example
    /// ```ignore
    /// let msg = Messages::low_effort_error(Language::English);
    /// assert!(msg.contains("detailed"));
    /// ```
    pub fn low_effort_error(lang: Language) -> String {
        match lang {
            Language::English => "Please provide a more detailed answer.".to_string(),
            Language::Ukrainian => "Будь ласка, дай більш детальну відповідь.".to_string(),
        }
    }

    /// Returns the error message for answers that are too short.
    ///
    /// # Arguments
    /// * `lang` - Selected language
    ///
    /// # Example
    /// ```ignore
    /// let msg = Messages::min_length_error(lang: Language) -> String {
    /// assert!(msg.contains("2 characters"));
    /// ```
    pub fn min_length_error(lang: Language) -> String {
        match lang {
            Language::English => "Your answer must be at least 2 characters long.".to_string(),
            Language::Ukrainian => "Твоя відповідь має містити хоча б 2 символи.".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Welcome message tests
    #[test]
    fn test_welcome_message_english() {
        let msg = Messages::welcome_message("Alice", "Rust Developers", Language::English);
        assert!(msg.contains("Hi Alice!"));
        assert!(msg.contains("Rust Developers"));
        assert!(msg.contains("moderator reviews it"));
        assert!(msg.contains("quick questions"));
    }

    #[test]
    fn test_welcome_message_ukrainian() {
        let msg = Messages::welcome_message("Алісія", "Rust Розробники", Language::Ukrainian);
        assert!(msg.contains("Привіт, Алісія!"));
        assert!(msg.contains("Rust Розробники"));
        assert!(msg.contains("модератор"));
        assert!(msg.contains("швидких питань"));
    }

    #[test]
    fn test_welcome_message_with_special_characters() {
        let msg = Messages::welcome_message("José", "C++ & Rust", Language::English);
        assert!(msg.contains("José"));
        assert!(msg.contains("C++ & Rust"));
    }

    // Language selection message tests
    #[test]
    fn test_language_selection_message() {
        let msg = Messages::language_selection_message("Bob", "Python Community");
        assert!(msg.contains("Bob"));
        assert!(msg.contains("Python Community"));
        assert!(msg.contains("English"));
        assert!(msg.contains("Українська"));
        assert!(msg.contains("🇬🇧"));
        assert!(msg.contains("🇺🇦"));
    }

    #[test]
    fn test_language_selection_message_bilingual() {
        let msg = Messages::language_selection_message("Charlie", "Web Dev");
        // Should contain both language options
        assert!(msg.contains("English"));
        assert!(msg.contains("Українська"));
    }

    // Completion message tests
    #[test]
    fn test_completion_message_english() {
        let msg = Messages::completion_message(Language::English);
        assert!(msg.contains("Thanks"));
        assert!(msg.contains("submitted"));
        assert!(msg.contains("moderators"));
        assert!(msg.contains("notified"));
    }

    #[test]
    fn test_completion_message_ukrainian() {
        let msg = Messages::completion_message(Language::Ukrainian);
        assert!(msg.contains("Дякую"));
        assert!(msg.contains("заявку"));
        assert!(msg.contains("модераторам"));
        assert!(msg.contains("повідомлення"));
    }

    // Required field error tests
    #[test]
    fn test_required_field_error_english() {
        let msg = Messages::required_field_error(Language::English);
        assert!(msg.contains("required"));
        assert!(msg.contains("answer"));
    }

    #[test]
    fn test_required_field_error_ukrainian() {
        let msg = Messages::required_field_error(Language::Ukrainian);
        assert!(msg.contains("обов'язкове"));
        assert!(msg.contains("відповідь"));
    }

    // Low effort error tests
    #[test]
    fn test_low_effort_error_english() {
        let msg = Messages::low_effort_error(Language::English);
        assert!(msg.contains("detailed"));
        assert!(msg.contains("answer"));
    }

    #[test]
    fn test_low_effort_error_ukrainian() {
        let msg = Messages::low_effort_error(Language::Ukrainian);
        assert!(msg.contains("детальну"));
        assert!(msg.contains("відповідь"));
    }

    // Min length error tests
    #[test]
    fn test_min_length_error_english() {
        let msg = Messages::min_length_error(Language::English);
        assert!(msg.contains("2 characters"));
        assert!(msg.contains("answer"));
    }

    #[test]
    fn test_min_length_error_ukrainian() {
        let msg = Messages::min_length_error(Language::Ukrainian);
        assert!(msg.contains("2 символи"));
        assert!(msg.contains("відповідь"));
    }

    // Integration tests
    #[test]
    fn test_all_messages_english_consistency() {
        // Verify all English messages are non-empty
        assert!(!Messages::welcome_message("Test", "Community", Language::English).is_empty());
        assert!(!Messages::completion_message(Language::English).is_empty());
        assert!(!Messages::required_field_error(Language::English).is_empty());
        assert!(!Messages::low_effort_error(Language::English).is_empty());
        assert!(!Messages::min_length_error(Language::English).is_empty());
    }

    #[test]
    fn test_all_messages_ukrainian_consistency() {
        // Verify all Ukrainian messages are non-empty
        assert!(!Messages::welcome_message("Тест", "Спільнота", Language::Ukrainian).is_empty());
        assert!(!Messages::completion_message(Language::Ukrainian).is_empty());
        assert!(!Messages::required_field_error(Language::Ukrainian).is_empty());
        assert!(!Messages::low_effort_error(Language::Ukrainian).is_empty());
        assert!(!Messages::min_length_error(Language::Ukrainian).is_empty());
    }

    #[test]
    fn test_language_selection_message_non_empty() {
        assert!(!Messages::language_selection_message("Test", "Community").is_empty());
    }

    #[test]
    fn test_welcome_message_formatting() {
        let msg = Messages::welcome_message("John", "DevOps", Language::English);
        // Should have newlines for proper formatting
        assert!(msg.contains("\n\n"));
    }

    #[test]
    fn test_welcome_message_ukrainian_formatting() {
        let msg = Messages::welcome_message("Іван", "DevOps", Language::Ukrainian);
        // Should have newlines for proper formatting
        assert!(msg.contains("\n\n"));
    }

    #[test]
    fn test_completion_message_formatting() {
        let msg = Messages::completion_message(Language::English);
        // Should have newline separating two sentences
        assert!(msg.contains("\n"));
    }

    #[test]
    fn test_completion_message_ukrainian_formatting() {
        let msg = Messages::completion_message(Language::Ukrainian);
        // Should have newline separating two sentences
        assert!(msg.contains("\n"));
    }
}
