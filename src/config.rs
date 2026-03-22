use serde::Deserialize;

use crate::error::ConfigError;

#[derive(Debug, Clone, Deserialize)]
pub struct Question {
    pub key: String,
    pub text_en: String,
    pub text_uk: String,
    pub required: bool,
    pub position: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommunityConfig {
    pub telegram_chat_id: i64,
    pub title: String,
    pub slug: String,
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotSettings {
    #[serde(default = "default_timeout")]
    pub application_timeout_minutes: u64,
    #[serde(default = "default_reminder")]
    pub reminder_before_expiry_minutes: u64,
}

fn default_timeout() -> u64 {
    60
}

fn default_reminder() -> u64 {
    15
}

impl Default for BotSettings {
    fn default() -> Self {
        Self {
            application_timeout_minutes: default_timeout(),
            reminder_before_expiry_minutes: default_reminder(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TomlConfig {
    #[serde(default)]
    bot: BotSettings,
    #[serde(default)]
    communities: Vec<CommunityConfig>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub database_url: String,
    pub default_moderator_chat_id: i64,
    pub allowed_moderator_ids: Vec<i64>,
    pub use_webhooks: bool,
    pub public_webhook_url: Option<String>,
    pub server_port: u16,
    pub rust_log: String,
    pub bot_settings: BotSettings,
    pub communities: Vec<CommunityConfig>,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from_env_and_toml(None)
    }

    pub fn load_from_env_and_toml(toml_content: Option<&str>) -> Result<Self, ConfigError> {
        let bot_token = require_env("BOT_TOKEN")?;
        let database_url = require_env("DATABASE_URL")?;
        let default_moderator_chat_id = require_env("DEFAULT_MODERATOR_CHAT_ID")?
            .parse::<i64>()
            .map_err(|e| ConfigError::InvalidEnvVar {
                name: "DEFAULT_MODERATOR_CHAT_ID".into(),
                reason: e.to_string(),
            })?;

        let allowed_moderator_ids = parse_comma_separated_i64(
            &require_env("ALLOWED_MODERATOR_IDS")?,
            "ALLOWED_MODERATOR_IDS",
        )?;

        let use_webhooks = std::env::var("USE_WEBHOOKS")
            .unwrap_or_else(|_| "false".into())
            .parse::<bool>()
            .map_err(|e| ConfigError::InvalidEnvVar {
                name: "USE_WEBHOOKS".into(),
                reason: e.to_string(),
            })?;

        let public_webhook_url = std::env::var("PUBLIC_WEBHOOK_URL").ok();

        let server_port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse::<u16>()
            .map_err(|e| ConfigError::InvalidEnvVar {
                name: "SERVER_PORT".into(),
                reason: e.to_string(),
            })?;

        let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "verifier_bot=info".into());

        let toml_str = match toml_content {
            Some(content) => content.to_string(),
            None => {
                let config_path =
                    std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".into());
                std::fs::read_to_string(&config_path)?
            }
        };

        let toml_cfg: TomlConfig = toml::from_str(&toml_str)?;

        let config = Self {
            bot_token,
            database_url,
            default_moderator_chat_id,
            allowed_moderator_ids,
            use_webhooks,
            public_webhook_url,
            server_port,
            rust_log,
            bot_settings: toml_cfg.bot,
            communities: toml_cfg.communities,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();

        if self.use_webhooks && self.public_webhook_url.is_none() {
            errors.push("PUBLIC_WEBHOOK_URL is required when USE_WEBHOOKS is true".into());
        }

        if self.communities.is_empty() {
            errors.push("at least one community must be configured".into());
        }

        let mut seen_slugs = std::collections::HashSet::new();
        for community in &self.communities {
            if !seen_slugs.insert(&community.slug) {
                errors.push(format!("duplicate community slug: {}", community.slug));
            }

            if community.questions.is_empty() {
                errors.push(format!(
                    "community '{}' must have at least one question",
                    community.slug
                ));
            }

            validate_question_positions(&community.slug, &community.questions, &mut errors);
            validate_question_texts(&community.slug, &community.questions, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Validation(errors))
        }
    }
}

fn require_env(name: &str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError::MissingEnvVar(name.into()))
}

fn parse_comma_separated_i64(value: &str, var_name: &str) -> Result<Vec<i64>, ConfigError> {
    value
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<i64>().map_err(|e| ConfigError::InvalidEnvVar {
                name: var_name.into(),
                reason: format!("cannot parse '{s}' as i64: {e}"),
            })
        })
        .collect()
}

fn validate_question_positions(slug: &str, questions: &[Question], errors: &mut Vec<String>) {
    let mut positions: Vec<u32> = questions.iter().map(|q| q.position).collect();
    positions.sort_unstable();
    positions.dedup();

    if positions.len() != questions.len() {
        errors.push(format!(
            "community '{slug}' has duplicate question positions"
        ));
        return;
    }

    let expected: Vec<u32> = (1..=questions.len() as u32).collect();
    if positions != expected {
        errors.push(format!(
            "community '{slug}' has gaps in question positions (expected 1..={}, got {positions:?})",
            questions.len()
        ));
    }
}

fn validate_question_texts(slug: &str, questions: &[Question], errors: &mut Vec<String>) {
    for question in questions {
        if question.text_en.trim().is_empty() {
            errors.push(format!(
                "community '{slug}' question '{}' has empty English text",
                question.key
            ));
        }
        if question.text_uk.trim().is_empty() {
            errors.push(format!(
                "community '{slug}' question '{}' has empty Ukrainian text",
                question.key
            ));
        }
    }
}
