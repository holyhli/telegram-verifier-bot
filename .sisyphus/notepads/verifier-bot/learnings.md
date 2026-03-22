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
