use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::fmt;

/// Supported languages for community questions and applicant sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Language {
    #[sqlx(rename = "en")]
    English,
    #[sqlx(rename = "uk")]
    Ukrainian,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Language::English => write!(f, "en"),
            Language::Ukrainian => write!(f, "uk"),
        }
    }
}

impl Language {
    /// Returns the database code for this language ('en' or 'uk').
    pub fn code(&self) -> &str {
        match self {
            Language::English => "en",
            Language::Ukrainian => "uk",
        }
    }

    /// Parses a language from its database code.
    /// Returns None if the code is not recognized.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Language::English),
            "uk" => Some(Language::Ukrainian),
            _ => None,
        }
    }

    /// Returns the human-readable name of this language.
    pub fn name(&self) -> &str {
        match self {
            Language::English => "English",
            Language::Ukrainian => "Ukrainian",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_code() {
        assert_eq!(Language::English.code(), "en");
        assert_eq!(Language::Ukrainian.code(), "uk");
    }

    #[test]
    fn test_language_from_code() {
        assert_eq!(Language::from_code("en"), Some(Language::English));
        assert_eq!(Language::from_code("uk"), Some(Language::Ukrainian));
        assert_eq!(Language::from_code("fr"), None);
        assert_eq!(Language::from_code(""), None);
    }

    #[test]
    fn test_language_name() {
        assert_eq!(Language::English.name(), "English");
        assert_eq!(Language::Ukrainian.name(), "Ukrainian");
    }

    #[test]
    fn test_language_display() {
        assert_eq!(Language::English.to_string(), "en");
        assert_eq!(Language::Ukrainian.to_string(), "uk");
    }

    #[test]
    fn test_language_equality() {
        assert_eq!(Language::English, Language::English);
        assert_ne!(Language::English, Language::Ukrainian);
    }

    #[test]
    fn test_language_copy() {
        let lang = Language::English;
        let lang_copy = lang;
        assert_eq!(lang, lang_copy);
    }
}
