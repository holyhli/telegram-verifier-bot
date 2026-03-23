# Learnings — stats-command

## Project Conventions
- Rust bot using teloxide 0.17, sqlx 0.8, tokio, axum
- Repository pattern: `src/db/` repos, `src/services/` business logic, `src/bot/handlers/` handlers
- TelegramApi trait in `src/bot/handlers/mod.rs` — all handlers use `&dyn TelegramApi` for testability
- FakeTelegramApi in each test file (NOT shared) — all test files have their own copy
- Tests use `#[sqlx::test(migrations = "./migrations")]` for fresh DB per test
- Test command: `DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all`
- sqlx compile-time checked queries — must run `cargo sqlx prepare` after adding new queries
- Enum casting in sqlx: `as "field: EnumType"` pattern (see join_request_repo.rs)
- No transactions anywhere in codebase
- Callback data prefixes: `lang:`, `a:`, `r:`, `b:` — new stats uses `sc:`, `sp:`, `sn:`
- Callback data 64-byte Telegram limit — use compact encoding
- Event instrumentation must be non-blocking: `if let Err(e) = ... { tracing::error!(...) }`

## Question Events Domain Model
- Created `migrations/011_create_question_events.sql` with audit trail table
- Schema: id, join_request_id, community_question_id, applicant_id, event_type (enum), metadata (JSONB), created_at
- Event types: QuestionPresented, ValidationFailed, AnswerAccepted
- Indexes on: join_request_id, (event_type, created_at), (community_question_id, created_at)
- No CASCADE DELETE (audit log should persist)
- Created `src/domain/question_event.rs` with QuestionEventType enum and QuestionEvent struct
- Enum derives: Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, sqlx::Type
- Enum sqlx attrs: `#[sqlx(type_name = "text", rename_all = "snake_case")]` (exact pattern from JoinRequestStatus)
- Struct derives: Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow
- Registered in `src/domain/mod.rs` with pub mod and re-exports
