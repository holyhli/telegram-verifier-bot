use tracing_subscriber::{fmt, EnvFilter};

pub fn init(rust_log: &str) {
    let env_filter = EnvFilter::try_new(rust_log).unwrap_or_else(|_| EnvFilter::new("info"));

    let is_production = std::env::var("ENVIRONMENT")
        .map(|v| v == "production")
        .unwrap_or(false);

    if is_production {
        fmt()
            .with_env_filter(env_filter)
            .json()
            .with_target(true)
            .with_thread_ids(false)
            .init();
    } else {
        fmt()
            .with_env_filter(env_filter)
            .pretty()
            .with_target(true)
            .with_thread_ids(false)
            .init();
    }
}
