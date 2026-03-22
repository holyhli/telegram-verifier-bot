use std::sync::Mutex;
use verifier_bot::config::Config;
use verifier_bot::error::ConfigError;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn set_required_env_vars() {
    std::env::set_var("BOT_TOKEN", "test_token_123");
    std::env::set_var("DATABASE_URL", "postgres://localhost/test");
    std::env::set_var("DEFAULT_MODERATOR_CHAT_ID", "-1001234567890");
    std::env::set_var("ALLOWED_MODERATOR_IDS", "111,222,333");
    std::env::set_var("USE_WEBHOOKS", "false");
}

fn clear_env_vars() {
    for var in [
        "BOT_TOKEN",
        "DATABASE_URL",
        "DEFAULT_MODERATOR_CHAT_ID",
        "ALLOWED_MODERATOR_IDS",
        "USE_WEBHOOKS",
        "PUBLIC_WEBHOOK_URL",
        "SERVER_PORT",
        "RUST_LOG",
        "CONFIG_PATH",
    ] {
        std::env::remove_var(var);
    }
}

const VALID_TOML: &str = r#"
[bot]
application_timeout_minutes = 30
reminder_before_expiry_minutes = 10

[[communities]]
telegram_chat_id = -1001234567890
title = "Test Community"
slug = "test-community"

[[communities.questions]]
key = "name"
text = "What is your name?"
required = true
position = 1

[[communities.questions]]
key = "reason"
text = "Why do you want to join?"
required = true
position = 2
"#;

#[test]
fn config_loads_valid_toml_and_env_vars() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();

    let config = Config::load_from_env_and_toml(Some(VALID_TOML)).unwrap();

    assert_eq!(config.bot_token, "test_token_123");
    assert_eq!(config.database_url, "postgres://localhost/test");
    assert_eq!(config.default_moderator_chat_id, -1001234567890);
    assert_eq!(config.allowed_moderator_ids, vec![111, 222, 333]);
    assert_eq!(config.bot_settings.application_timeout_minutes, 30);
    assert_eq!(config.communities.len(), 1);
    assert_eq!(config.communities[0].slug, "test-community");
    assert_eq!(config.communities[0].questions.len(), 2);
}

#[test]
fn config_rejects_missing_bot_token() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    std::env::set_var("DATABASE_URL", "postgres://localhost/test");
    std::env::set_var("DEFAULT_MODERATOR_CHAT_ID", "-100123");
    std::env::set_var("ALLOWED_MODERATOR_IDS", "111");

    let err = Config::load_from_env_and_toml(Some(VALID_TOML)).unwrap_err();
    match err {
        ConfigError::MissingEnvVar(name) => assert_eq!(name, "BOT_TOKEN"),
        other => panic!("expected MissingEnvVar, got: {other}"),
    }
}

#[test]
fn config_rejects_duplicate_community_slugs() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();

    let toml_with_dupes = r#"
[[communities]]
telegram_chat_id = -1001111111111
title = "Community A"
slug = "same-slug"

[[communities.questions]]
key = "q1"
text = "Question?"
required = true
position = 1

[[communities]]
telegram_chat_id = -1002222222222
title = "Community B"
slug = "same-slug"

[[communities.questions]]
key = "q1"
text = "Question?"
required = true
position = 1
"#;

    let err = Config::load_from_env_and_toml(Some(toml_with_dupes)).unwrap_err();
    match err {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .iter()
                    .any(|e| e.contains("duplicate community slug")),
                "expected duplicate slug error, got: {errors:?}"
            );
        }
        other => panic!("expected Validation error, got: {other}"),
    }
}

#[test]
fn config_rejects_gaps_in_question_positions() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();

    let toml_with_gaps = r#"
[[communities]]
telegram_chat_id = -1001234567890
title = "Test"
slug = "test"

[[communities.questions]]
key = "q1"
text = "First?"
required = true
position = 1

[[communities.questions]]
key = "q2"
text = "Third?"
required = true
position = 3
"#;

    let err = Config::load_from_env_and_toml(Some(toml_with_gaps)).unwrap_err();
    match err {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .iter()
                    .any(|e| e.contains("gaps in question positions")),
                "expected position gap error, got: {errors:?}"
            );
        }
        other => panic!("expected Validation error, got: {other}"),
    }
}

#[test]
fn config_parses_allowed_moderator_ids_from_comma_separated_string() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();
    std::env::set_var("ALLOWED_MODERATOR_IDS", "123, 456, 789");

    let config = Config::load_from_env_and_toml(Some(VALID_TOML)).unwrap();
    assert_eq!(config.allowed_moderator_ids, vec![123, 456, 789]);
}

#[test]
fn config_rejects_invalid_moderator_ids() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();
    std::env::set_var("ALLOWED_MODERATOR_IDS", "123,not_a_number,789");

    let err = Config::load_from_env_and_toml(Some(VALID_TOML)).unwrap_err();
    match err {
        ConfigError::InvalidEnvVar { name, reason } => {
            assert_eq!(name, "ALLOWED_MODERATOR_IDS");
            assert!(reason.contains("not_a_number"), "reason: {reason}");
        }
        other => panic!("expected InvalidEnvVar, got: {other}"),
    }
}

#[test]
fn config_rejects_invalid_toml_syntax() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();

    let bad_toml = "this is not valid toml [[[";

    let err = Config::load_from_env_and_toml(Some(bad_toml)).unwrap_err();
    assert!(
        matches!(err, ConfigError::TomlParseError(_)),
        "expected TomlParseError, got: {err}"
    );
}

#[test]
fn config_uses_default_bot_settings_when_omitted() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();

    let minimal_toml = r#"
[[communities]]
telegram_chat_id = -1001234567890
title = "Minimal"
slug = "minimal"

[[communities.questions]]
key = "q1"
text = "Question?"
required = true
position = 1
"#;

    let config = Config::load_from_env_and_toml(Some(minimal_toml)).unwrap();
    assert_eq!(config.bot_settings.application_timeout_minutes, 60);
    assert_eq!(config.bot_settings.reminder_before_expiry_minutes, 15);
}

#[test]
fn config_requires_webhook_url_when_webhooks_enabled() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();
    std::env::set_var("USE_WEBHOOKS", "true");
    std::env::remove_var("PUBLIC_WEBHOOK_URL");

    let err = Config::load_from_env_and_toml(Some(VALID_TOML)).unwrap_err();
    match err {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .iter()
                    .any(|e| e.contains("PUBLIC_WEBHOOK_URL is required")),
                "expected webhook URL error, got: {errors:?}"
            );
        }
        other => panic!("expected Validation error, got: {other}"),
    }
}

#[test]
fn config_rejects_empty_communities_list() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env_vars();
    set_required_env_vars();

    let empty_communities = r#"
[bot]
application_timeout_minutes = 30
"#;

    let err = Config::load_from_env_and_toml(Some(empty_communities)).unwrap_err();
    match err {
        ConfigError::Validation(errors) => {
            assert!(
                errors.iter().any(|e| e.contains("at least one community")),
                "expected empty communities error, got: {errors:?}"
            );
        }
        other => panic!("expected Validation error, got: {other}"),
    }
}
