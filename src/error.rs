use std::fmt;

#[derive(Debug)]
pub enum ConfigError {
    MissingEnvVar(String),
    InvalidEnvVar { name: String, reason: String },
    TomlReadError(std::io::Error),
    TomlParseError(toml::de::Error),
    Validation(Vec<String>),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnvVar(name) => write!(f, "missing required environment variable: {name}"),
            Self::InvalidEnvVar { name, reason } => {
                write!(f, "invalid environment variable {name}: {reason}")
            }
            Self::TomlReadError(err) => write!(f, "failed to read TOML config file: {err}"),
            Self::TomlParseError(err) => write!(f, "failed to parse TOML config: {err}"),
            Self::Validation(errors) => {
                write!(f, "config validation failed: {}", errors.join("; "))
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TomlReadError(err) => Some(err),
            Self::TomlParseError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        Self::TomlReadError(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        Self::TomlParseError(err)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("telegram error: {0}")]
    Telegram(String),

    #[error("{0}")]
    Internal(String),
}
