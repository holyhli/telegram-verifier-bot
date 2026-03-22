# Learnings: verifier-bot

This file tracks conventions, patterns, and architectural decisions discovered during implementation.

---

## [2026-03-22] Task 1: Project Foundation

### Crate Versions Used
- teloxide = "0.17.0" (latest stable, features: macros)
- sqlx = "0.8.6" (latest stable; 0.9.0-alpha.1 exists but skipped for stability)
- tokio = "1.50.0" (features: full)
- serde = "1.0.228" (features: derive)
- toml = "1.0.7" (was 0.8.x series, now jumped to 1.0 — uses toml_parser internally)
- dotenvy = "0.15.7" (well-maintained dotenv fork)
- chrono = "0.4.44" (features: serde)
- uuid = "1.22.0" (features: v4, serde)
- axum = "0.8.8"
- thiserror = "2.0.18" (major version 2.x — uses derive macros differently from 1.x)
- anyhow = "1.0.102"
- tracing = "0.1.44"
- tracing-subscriber = "0.3.23" (features: env-filter, json)

### Config Pattern Decisions
- Two-source config: env vars (secrets) + TOML file (community definitions + bot settings)
- `Config::load_from_env_and_toml(toml_content: Option<&str>)` allows tests to inject TOML directly without filesystem
- Validation runs after both sources are merged — catches cross-source issues like webhooks enabled without URL
- `ConfigError` is a manual enum (not thiserror) to keep error.rs clean; `AppError` uses thiserror
- Question positions must be contiguous 1..=N with no gaps or duplicates

### Testing Pattern
- Tests use `Mutex<()>` to serialize env var access (env vars are process-global)
- `set_var`/`remove_var` used in tests — safe because tests hold the mutex
- 10 config tests covering: valid parse, missing env vars, duplicate slugs, position gaps, comma-separated ID parsing, invalid IDs, bad TOML, default settings, webhook validation, empty communities

### Gotchas Discovered
- rust-analyzer not installed in this toolchain — `cargo build` is the definitive compiler check
- toml crate version jumped from 0.8.x to 1.0.7 — the "+spec-1.1.0" suffix in version is metadata indicating TOML spec compliance
- teloxide 0.17.0 uses `Bot::from_env()` which reads `TELOXIDE_TOKEN` by default — we'll construct Bot manually with our config's `bot_token` field
- thiserror 2.x changes derive syntax slightly from 1.x but our usage works fine
