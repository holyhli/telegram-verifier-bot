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

## [2026-03-22] Task 2: Database Schema

### Migration Structure
- 9 migration files in `migrations/` directory, named `001_create_communities.sql` through `009_add_reminder_sent_at.sql`
- All migrations use `IF NOT EXISTS` / `IF NOT EXISTS` for idempotency
- CHECK constraints used for status enums instead of PostgreSQL custom types — simpler to manage, sqlx handles them as TEXT columns, no custom type mapping needed

### Config Sync Pattern
- `sync_config_to_db()` takes `&PgPool` and `&[CommunityConfig]`, iterates communities
- Upserts communities via `ON CONFLICT (telegram_chat_id) DO UPDATE` — telegram_chat_id is the natural key
- Upserts questions via `ON CONFLICT (community_id, question_key) WHERE is_active = TRUE` — partial unique index used as conflict target
- Questions no longer in TOML are deactivated with `question_key != ALL($2)` — uses PostgreSQL array operator to match against list of active keys

### Constraints & Indexes
- Unique partial index `uq_join_requests_active_per_applicant_community` on `(applicant_id, community_id) WHERE status NOT IN (...)` — prevents duplicate active requests while allowing re-application after approval/rejection
- All constraints are named for debuggable error messages (e.g., `chk_join_requests_status`)
- Blacklist entries have cross-field CHECK: `scope_type = 'global'` requires `community_id IS NULL`, `scope_type = 'community'` requires `community_id IS NOT NULL`
- Community questions have partial unique indexes scoped to `is_active = TRUE` — allows deactivated questions to coexist with new ones at same position/key

### Offline Query Metadata
- `cargo sqlx prepare` generates `.sqlx/` directory with JSON files (one per `query!`/`query_scalar!` macro invocation)
- 3 query metadata files generated for the 3 compile-time-checked queries in `sync.rs`
- `SQLX_OFFLINE=true cargo build` works without a database connection — required for Docker multi-stage builds
- `.sqlx/` must be committed to git (not in `.gitignore`)

### Testing Pattern
- `#[sqlx::test]` creates an isolated test database per test function — auto-applies all migrations from `./migrations`
- Requires `DATABASE_URL` env var pointing to a PostgreSQL server (not the specific database — sqlx creates temporary DBs)
- 7 tests: migrations apply, sync creates, sync updates, sync deactivates, duplicate active rejected, invalid status rejected, approved allows new
- `cargo test --test db_tests` runs all db tests; `cargo test db` only matches test names containing "db"

### Gotchas Discovered
- sqlx `query!` macro validates against live DB at compile time OR falls back to `.sqlx/` offline metadata when `SQLX_OFFLINE=true`
- Partial unique indexes work as ON CONFLICT targets in PostgreSQL — the `WHERE is_active = TRUE` clause on the unique index for `(community_id, question_key)` is correctly used as ON CONFLICT target
- `updated_at` is set application-side (in upsert queries) rather than via PostgreSQL triggers — simpler, avoids trigger management in migrations
- `telegram_*_id` fields are all BIGINT — Telegram IDs exceed i32 range (up to 2^52)

## [2026-03-22] Task 3: Database Tests Implementation

### Test Suite Verification
- All 7 `#[sqlx::test]` tests pass consistently
- Test database setup: `CREATE DATABASE verifier_bot_test` required before test execution
- Execution: `DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot_test" cargo test --test db_tests`

### Test Coverage Details
1. **migrations_apply_to_fresh_db**: Validates all 8 tables exist (communities, community_questions, applicants, join_requests, join_request_answers, moderation_actions, blacklist_entries, applicant_sessions)
2. **sync_creates_communities_and_questions**: Tests initial sync with 2 questions, verifies community count=1 and question count=2
3. **sync_updates_existing_community**: Tests UPSERT — title updated, question text updated, community count remains 1 (no duplicate)
4. **sync_deactivates_removed_questions**: Tests partial deactivation — removed question marked `is_active=false`, kept question stays `is_active=true`
5. **duplicate_active_join_request_rejected**: Tests unique constraint — second active request for same applicant+community fails with error
6. **invalid_status_rejected_by_check_constraint**: Tests CHECK constraint — invalid status value rejected, error message contains constraint name `chk_join_requests_status`
7. **approved_request_allows_new_active_request**: Tests constraint logic — approved status doesn't block new active request (constraint only prevents multiple active)

### Test Isolation & Independence
- Each test gets fresh schema via `#[sqlx::test]` isolation
- No shared state between tests
- Tests use raw SQL inserts for setup (not sync_config_to_db) to test constraint behavior directly
- Async/await pattern: all tests are `async fn` with `sqlx::Result<()>` return type

### Key Patterns Observed
- Tests insert test data directly with `sqlx::query()` for constraint testing
- Constraint violations tested via `.is_err()` checks on query results
- Error messages validated with `.to_string().contains()` for constraint name verification
- All 7 tests independent — can run individually or as suite

## [2026-03-22] Task 3: Domain Models + Repository Layer

### Domain Model Decisions
- 7 domain model files in `src/domain/` (community.rs has both Community + CommunityQuestion)
- Enum derives: `Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type`
- Struct derives: `Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow`
- All enums use `#[sqlx(type_name = "text", rename_all = "snake_case")]` for TEXT column mapping
- All enums implement `Display` returning the snake_case DB value

### Repository Patterns
- Repos are zero-sized structs with associated async functions taking `&PgPool` as first param
- All return `Result<T, AppError>` — sqlx::Error auto-converts via `#[from]`
- `query_as!` used for all queries; enum columns require type override: `status as "status: JoinRequestStatus"`
- RETURNING clauses must list all columns explicitly (can't use `RETURNING *` with type overrides)
- `fetch_optional` for nullable lookups, `fetch_one` for required, `fetch_all` for lists

### Optimistic Locking Implementation
- `JoinRequestRepo::update_status()` WHERE clause checks `id AND status AND updated_at`
- Validates transition with `can_transition_to()` before hitting DB (fast-fail for invalid transitions)
- Returns `AppError::AlreadyProcessed` when 0 rows affected (concurrent modification detected)
- `updated_at` column set to `NOW()` on each status change — serves as the version field

### Error Handling
- `AppError` expanded with: `InvalidStateTransition`, `NotFound`, `Unauthorized`, `AlreadyProcessed`
- `ConfigError` kept as manual enum (from Task 1), wrapped by `AppError::Config(#[from])`
- `InvalidStateTransition` and `AlreadyProcessed` carry the enum values for actionable error messages

### Testing Patterns
- 27 tests total: 11 unit tests (status transitions), 16 `#[sqlx::test]` integration tests
- Seed helper functions (`seed_community`, `seed_applicant`, `seed_community_question`) keep tests DRY
- Optimistic locking tested by: create → update (changes updated_at) → retry with stale timestamp → assert AlreadyProcessed
- Idempotency tested by: create applicant → upsert same telegram_user_id → assert same id, updated fields

### Gotchas Discovered
- `query_as!` macro expands to code referencing type override names directly — the imported type (e.g. `SessionState`) must be in scope even if not visibly used in the source file; requires `#[allow(unused_imports)]`
- `#[sqlx(type_name = "text")]` on enums is needed for TEXT column compatibility — without it, sqlx tries to match a PostgreSQL custom type
- `rename_all = "snake_case"` correctly handles multi-word variants: `QuestionnaireInProgress` → `questionnaire_in_progress`
- `cargo sqlx prepare` must run AFTER migrations exist in the database — compile-time macros validate against live schema
- `find_expired` cutoff logic: `created_at < cutoff` means cutoff must be MORE RECENT than records to find them (not older)
- `SQLX_OFFLINE=true cargo build` works once `.sqlx/` cache is generated — all 24 new query metadata files cached

## [2026-03-22] Task 4: Bot Dispatcher + Join Request Handler

### Dispatcher Setup
- `Dispatcher::builder` now wires three top-level branches: `chat_join_request`, private `message`, and `callback_query`
- Race-condition prevention is enabled via `.distribution_function(|upd: &Update| upd.from().map(|user| user.id.0))`, so all updates for one user are serialized
- Dependencies are injected with `teloxide::dptree::deps![pool, Arc::new(config)]` to make `PgPool` and runtime config available in handlers
- Long polling uses explicit `allowed_updates` for `Message`, `CallbackQuery`, and `ChatJoinRequest`

### Handler Patterns
- Join request logic is split into teloxide adapter (`handle_join_request`) + testable core (`process_join_request`)
- Flow order is: community lookup → blacklist check (+ decline) → applicant upsert → duplicate-active guard → create join request → send first message immediately → create session → status transition to `questionnaire_in_progress`
- User-unreachable send failures (`BotBlocked`, `UserDeactivated`, `ChatNotFound`, or equivalent `Unknown` forbidden text) mark the join request `cancelled`; other send errors are logged and left `pending_contact`
- `/start` fallback similarly uses thin adapter + core function, resumes only `pending_contact` requests, then creates session and transitions state
- Logging uses structured fields (`join_request_id`, `community_id`, `telegram_user_id`) at success and failure points

### Testing Approach
- Added `tests/handler_tests.rs` with a `FakeTelegramApi` implementing a shared `TelegramApi` trait, so handlers are tested without real Telegram requests
- Tests run as `#[sqlx::test]` integration tests against real schema and repos, while Telegram side effects are captured in memory
- 7 scenarios covered: happy path creation, unknown community no-op, blacklist decline, duplicate idempotency, `/start` resume, `/start` generic reply, and blocked-user cancellation path

### Gotchas Discovered
- `teloxide::ApiError` is exported at crate root (`teloxide::ApiError`), not under `teloxide::types`
- `update_listeners::polling_default` in 0.17 returns a ready listener future; for explicit `allowed_updates`, `Polling::builder(bot)` is the simpler path
- `cargo test handler` can filter out all tests if names do not match as expected; using `cargo test --test handler_tests` is the reliable task-level check

## [2026-03-22] Task 5: Questionnaire FSM + Answer Persistence

### Service/Handler Split
- Added `src/services/questionnaire.rs` for business logic and state transitions, and `src/bot/handlers/questionnaire.rs` as a thin Telegram adapter.
- `process_private_message` in the handler only does routing: active session lookup, send validation errors, send next question, or send completion text.
- FSM and persistence are centralized in `process_answer`: validate -> persist answer -> advance session OR complete session and transition join request to `submitted`.

### Validation Rules Locked In
- `validate_answer()` trims input and enforces: required non-empty, required min length >= 2 chars, and low-effort blocklist (`.", "..", "x", "xx", "test", "asdf", "123", "aaa", "-", "no", "n/a"`) case-insensitively.
- Optional questions allow empty/whitespace-only answers (stored as empty string).
- Error messages are exact constants to match product copy and test assertions.

### Active Session Lookup Pattern
- For private answers, active context is loaded via one SQL join across `applicants`, `join_requests`, `applicant_sessions`, and `community_questions` keyed by `telegram_user_id`.
- Query constrains to `join_requests.status = 'questionnaire_in_progress'`, `applicant_sessions.state = 'awaiting_answer'`, and active question at current position.
- If no active context exists, private messages are ignored (no DB writes, no outbound message).

### Completion Behavior
- On final answer: session state changes to `completed`, join request transitions `questionnaire_in_progress -> submitted`, applicant receives completion message, and handler logs `TODO: send moderator card` (Task 6 stub).
- No moderation approve/decline logic is triggered in this phase; completion only submits for moderator review.

### Testing Coverage
- Added `tests/questionnaire_tests.rs` with 11 tests covering validation outcomes, persistence, non-advancement on invalid answers, completion path, no-session ignore, and full 5-question flow with final status assertion (`submitted`).
- `cargo test questionnaire` runs tests whose names contain `questionnaire` across multiple test files (not just `questionnaire_tests.rs`).
- `#[sqlx::test]` requires `DATABASE_URL` to be set; command used in this repo remains: `DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot_test" cargo test ...`.

## [2026-03-22] Task 6: Moderator Card + Inline Actions + Audit Trail

### Moderator Card Delivery
- Added `src/services/moderator.rs` with `render_moderator_card()` and `send_moderator_card()`.
- Card payload is rendered in HTML with escaped user-provided fields and compact callback payloads (`a:{id}`, `r:{id}`, `b:{id}`).
- `send_moderator_card()` now sends HTML + inline keyboard to `default_moderator_chat_id` and persists `submitted_to_moderators_at`, `moderator_message_chat_id`, and `moderator_message_id` on the join request.

### Callback Moderation Flow
- Replaced callback stub with `src/bot/handlers/callbacks.rs` and a testable core entrypoint `process_callback_query()`.
- Flow is: parse callback data -> moderator authorization (`allowed_moderator_ids`) -> status guard (`submitted`) -> optimistic lock transition -> Telegram approve/decline -> audit insert -> card edit + keyboard removal -> callback acknowledgement.
- Ban action is implemented as decline + blacklist entry (`ScopeType::Community`) and does not call `banChatMember`.

### Telegram API Error Handling
- Added friendly callback handling for invalid payload, unauthorized moderator, already-processed joins, and Telegram `HIDE_REQUESTER_MISSING` (reported as "Request already processed outside bot").
- Applicant notification failures from blocked/deactivated/unreachable users are treated as non-fatal so moderation completion still persists.

### API Abstraction Update
- Extended `TelegramApi` trait for moderation operations: HTML send/edit, callback answer, approve request, and reply-markup clearing.
- `TeloxideApi` now uses teloxide setter traits (`SendMessageSetters`, `EditMessageTextSetters`, `AnswerCallbackQuerySetters`) for parse mode and callback text.

### Testing Coverage
- Added `tests/moderation_tests.rs` with 12 moderation-specific tests (>=10 required): card rendering, callback parsing, authorization checks, approve/reject/ban flows, optimistic double-processing behavior, and Telegram error paths.
- Full verification command used: `DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot_test" cargo test --all`.
