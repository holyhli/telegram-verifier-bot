# Telegram Join Request Moderation Bot (verifier-bot)

## TL;DR

> **Quick Summary**: Build a Rust Telegram moderation bot using teloxide + sqlx + tokio that processes join requests across multiple communities, conducts private onboarding questionnaires, stores answers in PostgreSQL, and enables moderators to approve/reject/ban via inline buttons in a dedicated chat. TDD throughout, Docker Compose deployment.
>
> **Deliverables**:
> - Rust binary (`verifier-bot`) with full join-request-to-decision lifecycle
> - 8-table PostgreSQL schema with embedded migrations
> - TOML-based community/questionnaire configuration
> - Docker Compose (bot + postgres) with multi-stage Dockerfile
> - Both long-polling and webhook support (switchable via env var)
> - README, `.env.example`, `config.example.toml`
>
> **Estimated Effort**: Large (10 tasks)
> **Parallel Execution**: YES — 7 waves, tasks 7/8 and 9/10 can parallelize
> **Critical Path**: Foundation → Schema → Models → Bot Core → Questionnaire FSM → Moderator Flow → Expiry/Webhook → Docker → Docs

---

## Context

### Original Request
Build a production-lean MVP Telegram moderation bot in Rust (detailed 30-section product spec provided). The bot handles join requests for multiple Telegram communities, messages applicants with configurable questionnaires, stores answers in PostgreSQL, forwards completed applications to moderators with inline approve/reject/ban buttons, and deploys via Docker Compose.

### Interview Summary
**Key Decisions**:
- **Test strategy**: TDD (Red-Green-Refactor) with `#[sqlx::test]` for DB tests + `teremock` for bot handler tests
- **Community registration**: TOML config file loaded at startup and synced (upserted) to PostgreSQL
- **Moderator allowlist**: Environment variable `ALLOWED_MODERATOR_IDS=123,456,789`
- **Update mode**: Both long polling AND webhook, switchable via `USE_WEBHOOKS` env var (requires axum)
- **Expiry**: Configurable timeout with one reminder message before expiry

**Research Findings**:
- **teloxide architecture**: `dptree::entry()` dispatcher with filter branches (`filter_chat_join_request`, `filter_message`, `filter_callback_query`); dependency injection via `dptree::deps![]`; repository pattern for DB (DickGrowerBot)
- **ChatJoinRequest critical**: `user_chat_id` is `ChatId` (NOT UserId) — **5-minute window** to message applicant after join request. Must send first contact immediately. After first message, private chat continues normally
- **Dialogue storage limitation**: teloxide only supports InMemory/Redis/SQLite storage — no PostgreSQL. Combined with dynamic per-community questions, this means we must build a custom FSM on PostgreSQL
- **sqlx**: `sqlx::migrate!()` embeds migrations at compile time; `query_as!` for type-safe queries; `#[sqlx::test]` for per-test isolated databases; `SQLX_OFFLINE=true` with `.sqlx/` directory for Docker builds
- **Docker**: cargo-chef for layer caching (10x faster rebuilds), debian:bookworm-slim runtime

### Metis Review
**Critical Issues Identified (all addressed in plan)**:
- **Race condition**: `chat_join_request` dispatches to group chat key, user's answer dispatches to private chat key → concurrent execution → session not yet committed. **Fix**: Set `distribution_function` keyed by `user_id` to serialize all updates per-user
- **Silent update loss**: Default `LoggingErrorHandler` discards failed updates. A lost `chat_join_request` means a user is permanently stuck. **Fix**: Custom error handler with retry logic for transient errors
- **Docker SIGTERM**: `.enable_ctrlc_handler()` only handles SIGINT, not SIGTERM (which Docker sends on `docker stop`). **Fix**: `ShutdownToken` + tokio signal handler for SIGTERM
- **Compile-time DB requirement**: `sqlx::query_as!` needs a running DB at compile time. **Fix**: `SQLX_OFFLINE=true` with checked-in `.sqlx/` directory
- **Telegram API errors**: 403 (user blocked bot) and 400 `HIDE_REQUESTER_MISSING` (request already processed by human admin) not handled. **Fix**: Specific error handling per API call with appropriate state transitions
- **Ban semantics**: Default applied — Ban = decline join request + add to `blacklist_entries` table (auto-declines future requests). No Telegram `banChatMember` API call for MVP
- **Moderator group topology**: Default applied — single flat supergroup for MVP (topic routing is future enhancement)

---

## Work Objectives

### Core Objective
Build a working Telegram moderation bot that handles the complete lifecycle: join request → private questionnaire → moderator review → approve/reject/ban, supporting multiple communities, with TDD, structured logging, and Docker Compose deployment.

### Concrete Deliverables
- `src/` — Rust source (~20 files across config/, bot/, domain/, db/, services/ modules)
- `migrations/` — 8+ PostgreSQL migration files
- `config.example.toml` — Community + questionnaire configuration template
- `.env.example` — Environment variable reference
- `Dockerfile` — Multi-stage production build with cargo-chef
- `docker-compose.yml` — Production deployment (bot + postgres)
- `docker-compose.dev.yml` — Development with test DB
- `README.md` — Setup, run, Telegram permissions guide
- `.sqlx/` — Offline query metadata for Docker builds
- `tests/` — Integration test suite

### Definition of Done
- [ ] `cargo test --all` passes with 0 failures
- [ ] `docker compose up` starts bot + postgres, bot connects and begins polling
- [ ] Bot processes join request → questionnaire → moderator card → approve/reject for ≥2 communities
- [ ] Stale applications expire with reminder message sent
- [ ] Moderator actions are audit-logged in `moderation_actions` table
- [ ] Blacklisted users are auto-declined on join request
- [ ] Webhook mode works when `USE_WEBHOOKS=true`

### Must Have
- Single bot handling all responsibilities (admin in communities + DMs applicants + posts to moderator chat)
- Custom questionnaire FSM on PostgreSQL (NOT teloxide's built-in dialogue system)
- `distribution_function` keyed by `user_id` in dispatcher (prevents race conditions)
- Custom error handler (prevents silent update loss)
- SIGTERM handling via `ShutdownToken` (Docker compatibility)
- `SQLX_OFFLINE=true` with `.sqlx/` directory checked into repo
- Idempotent join request creation (duplicate Telegram updates handled safely)
- Double-processing protection with optimistic concurrency on moderation actions
- Structured logging via `tracing` with contextual fields (join_request_id, community_id, user_id)
- Specific error handling for Telegram API failures (403 blocked → mark cancelled; 400 HIDE_REQUESTER_MISSING → mark externally processed)
- `allowed_updates` set explicitly to `["message", "callback_query", "chat_join_request"]`
- `/start` command handler as fallback for users who didn't receive automatic message

### Must NOT Have (Guardrails)
- Do NOT use teloxide's built-in dialogue system (lacks PostgreSQL + dynamic questions)
- Do NOT approve/decline join request during questionnaire — only after moderator acts
- Do NOT use UUIDs in callback_data (64-byte limit after URL encoding)
- Do NOT use `.enable_ctrlc_handler()` alone — must add SIGTERM via `ShutdownToken`
- Do NOT put business logic beyond "create session + send Q1" in the `chat_join_request` handler
- Do NOT log raw applicant answers at debug level (privacy)
- Do NOT expose PostgreSQL externally in Docker Compose
- Do NOT store bot token or DB credentials in config files — env vars only for secrets
- Do NOT create a second bot
- Do NOT add unnecessary abstraction (no DDD, no CQRS, no event sourcing, no generic traits where concrete impls suffice)
- Do NOT build: web dashboard, OAuth, AI classification, CRM integration, analytics, payments, public status page

---

## Verification Strategy

> **UNIVERSAL RULE: ZERO HUMAN INTERVENTION**
> ALL verification is executed by the agent using tools (Bash, interactive_bash/tmux).
> No manual Telegram interaction required for acceptance.

### Test Decision
- **Infrastructure exists**: NO (greenfield — setup in Task 1)
- **Automated tests**: TDD (Red-Green-Refactor)
- **Framework**: `cargo test` (built-in) + `#[sqlx::test]` for DB + `teremock` for bot handlers

### TDD Workflow Per Task

Each TODO follows RED-GREEN-REFACTOR:

1. **RED**: Write failing test first
   - Test file: `tests/<module>.rs` or inline `#[cfg(test)] mod tests`
   - Command: `cargo test <test_name>` → FAIL (test exists, implementation doesn't)
2. **GREEN**: Implement minimum code to pass
   - Command: `cargo test <test_name>` → PASS
3. **REFACTOR**: Clean up while keeping green
   - Command: `cargo test --all` → PASS (still)

### Test Infrastructure Setup (Part of Task 1)
- `docker-compose.dev.yml` with PostgreSQL for test DB
- `#[sqlx::test]` attribute for DB integration tests (auto per-test DB isolation + migration)
- `teremock` crate for teloxide handler integration tests
- Pure function unit tests for business logic (no framework dependency)
- `SQLX_OFFLINE=true` + `cargo sqlx prepare` for `.sqlx/` directory generation

### Agent-Executed QA Scenarios (MANDATORY — ALL tasks)

| Deliverable Type | Tool | How Agent Verifies |
|-----------------|------|-------------------|
| Rust compilation | Bash | `cargo build` exits 0, no errors |
| Unit/integration tests | Bash | `cargo test --all` passes |
| Config validation | Bash (tmux) | Run binary with invalid config, assert stderr contains error |
| Docker startup | Bash | `docker compose up -d && sleep 5 && docker compose ps` shows healthy |
| Health endpoint | Bash (curl) | `curl -sf localhost:8080/health` returns `{"status":"ok"}` |
| Migration | Bash | `sqlx migrate run` succeeds with test DB |
| Binary behavior | Bash (tmux) | Start binary, capture logs, verify expected log lines |

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately):
└── Task 1: Project Foundation + Config + Test Infrastructure

Wave 2 (After Wave 1):
└── Task 2: Database Schema + Migrations

Wave 3 (After Wave 2):
└── Task 3: Domain Models + Repository Layer

Wave 4 (After Wave 3 — sequential pair):
├── Task 4: Bot Dispatcher + Join Request Handler
└── Task 5: Questionnaire FSM + Answer Persistence (after Task 4)

Wave 5 (After Wave 4):
└── Task 6: Moderator Card + Inline Actions + Audit Trail

Wave 6 (After Wave 5 — parallel pair):
├── Task 7: Expiry System + Blacklist + Reminder
└── Task 8: Webhook Support + Graceful Shutdown + Error Handling

Wave 7 (After Wave 6 — parallel pair):
├── Task 9: Docker Production Build
└── Task 10: Documentation + Polish

Critical Path: 1 → 2 → 3 → 4 → 5 → 6 → {7,8} → {9,10}
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 | None | 2 | None (foundation) |
| 2 | 1 | 3 | None |
| 3 | 2 | 4 | None |
| 4 | 3 | 5 | None |
| 5 | 4 | 6 | None |
| 6 | 5 | 7, 8 | None |
| 7 | 6 | 9, 10 | 8 |
| 8 | 6 | 9, 10 | 7 |
| 9 | 7, 8 | None | 10 |
| 10 | 7, 8 | None | 9 |

### Agent Dispatch Summary

| Wave | Tasks | Agents |
|------|-------|--------|
| 1 | 1 | `task(category="unspecified-high", load_skills=[], ...)` |
| 2 | 2 | `task(category="unspecified-high", load_skills=[], ...)` |
| 3 | 3 | `task(category="unspecified-high", load_skills=[], ...)` |
| 4 | 4→5 | `task(category="deep", load_skills=[], ...)` — async + FSM complexity |
| 5 | 6 | `task(category="deep", load_skills=[], ...)` — callback + concurrency |
| 6 | 7, 8 | Two parallel `task(category="unspecified-high", load_skills=[], ...)` |
| 7 | 9, 10 | `task(category="quick", ...)` Docker; `task(category="writing", ...)` docs |

---

## TODOs

> Every task includes TDD (RED-GREEN-REFACTOR) + Agent-Executed QA.
> The user's product spec (30 sections) is the primary reference — tasks reference sections by number.
> **CRITICAL**: This is a greenfield project. All file paths are to-be-created, not existing.

---

- [ ] 1. Project Foundation + Config + Test Infrastructure

  **What to do**:

  **1a. Project scaffolding**:
  - Initialize Rust project: `cargo init --name verifier-bot`
  - Create directory structure:
    ```
    src/
    ├── main.rs
    ├── config.rs
    ├── bot/
    │   ├── mod.rs
    │   └── handlers/
    │       └── mod.rs
    ├── domain/
    │   └── mod.rs
    ├── db/
    │   └── mod.rs
    ├── services/
    │   └── mod.rs
    └── error.rs
    migrations/
    tests/
    ```
  - Set up `Cargo.toml` with all dependencies:
    ```toml
    [dependencies]
    teloxide = { version = "0.13", features = ["macros"] }
    tokio = { version = "1", features = ["full"] }
    sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "chrono", "uuid", "migrate"] }
    serde = { version = "1", features = ["derive"] }
    serde_json = "1"
    toml = "0.8"
    chrono = { version = "0.4", features = ["serde"] }
    uuid = { version = "1", features = ["v4", "serde"] }
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
    thiserror = "2"
    anyhow = "1"
    dotenvy = "0.15"
    axum = "0.7"           # For webhook mode
    tower = "0.5"
    tokio-util = "0.7"

    [dev-dependencies]
    sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "chrono", "uuid", "migrate"] }
    # teremock or equivalent for bot testing — investigate current best option
    ```
  - **IMPORTANT**: Check crates.io for the LATEST versions of all crates (especially teloxide — it may be 0.13, 0.14, or newer). Do NOT blindly use the versions above. Run `cargo search teloxide`, `cargo search sqlx`, etc. or check crates.io to find the current latest versions and use those.

  **1b. Configuration system**:
  - Create `src/config.rs` with two config sources:
    - **Environment variables** (secrets): `BOT_TOKEN`, `DATABASE_URL`, `ALLOWED_MODERATOR_IDS`, `DEFAULT_MODERATOR_CHAT_ID`, `APPLICATION_TIMEOUT_MINUTES`, `REMINDER_BEFORE_EXPIRY_MINUTES`, `USE_WEBHOOKS`, `PUBLIC_WEBHOOK_URL`, `SERVER_PORT`, `RUST_LOG`, `CONFIG_PATH`
    - **TOML file** (community config): Communities + questionnaire definitions
  - TOML config structure:
    ```toml
    [bot]
    application_timeout_minutes = 60
    reminder_before_expiry_minutes = 15

    [[communities]]
    telegram_chat_id = -1001234567890
    title = "DeFi Amsterdam"
    slug = "defi-amsterdam"

    [[communities.questions]]
    key = "name"
    text = "What is your name?"
    required = true
    position = 1

    [[communities.questions]]
    key = "occupation"
    text = "What do you do / where do you work?"
    required = true
    position = 2
    ```
  - Implement config validation: reject missing required fields, duplicate slugs, gaps in question positions
  - Use `dotenvy` for `.env` loading, `toml` crate for TOML parsing, `serde` for deserialization
  - Parse `ALLOWED_MODERATOR_IDS` from comma-separated string to `Vec<i64>`

  **1c. Test infrastructure**:
  - Create `docker-compose.dev.yml` with PostgreSQL for local dev/test:
    ```yaml
    services:
      postgres:
        image: postgres:16-alpine
        environment:
          POSTGRES_USER: verifier_bot
          POSTGRES_PASSWORD: verifier_bot
          POSTGRES_DB: verifier_bot_test
        ports:
          - "5433:5432"
        healthcheck:
          test: ["CMD-SHELL", "pg_isready -U verifier_bot"]
          interval: 5s
          timeout: 3s
          retries: 5
    ```
  - Create `.env.test` with test database URL
  - Verify `#[sqlx::test]` works with a trivial test
  - Create `.env.example` with all env vars documented
  - Create `config.example.toml` with two example communities

  **1d. Logging setup**:
  - Initialize `tracing-subscriber` with `EnvFilter` in `main.rs`
  - JSON format for production, pretty format for development (based on env var)

  **TDD**:
  - **RED**: Write tests for config parsing — valid TOML, missing fields, invalid data, env var parsing, moderator ID parsing
  - **GREEN**: Implement `Config` struct + `Config::load()` function
  - **REFACTOR**: Extract validation into separate functions

  **Must NOT do**:
  - Do NOT hardcode any configuration values
  - Do NOT put bot token or DB URL in TOML — env vars only for secrets
  - Do NOT use `config` crate if plain `toml` + `dotenvy` is simpler
  - Do NOT write handlers or domain logic yet — only config + project skeleton

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Scaffolding + config + test infra is foundational work, not domain-specific
  - **Skills**: `[]`
    - No specialized skills needed — standard Rust project setup
  - **Skills Evaluated but Omitted**:
    - `playwright`: No browser work
    - `frontend-ui-ux`: No UI

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (solo)
  - **Blocks**: Task 2
  - **Blocked By**: None (first task)

  **References**:
  - **User spec Section 17**: Rust technical requirements — stack, project structure, env vars
  - **User spec Section 8**: Questionnaire requirements — needed for TOML config structure
  - **User spec Section 14**: Multi-community support — each community has own questionnaire
  - **Research finding (sqlx)**: `sqlx::migrate!()` needs migrations directory; `SQLX_OFFLINE=true` needs `.sqlx/` dir
  - **Research finding (teloxide)**: Crate features to enable — check `teloxide` feature flags for `macros`, `ctrlc_handler` (but we'll use custom shutdown)

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test config` → ≥5 tests pass (valid TOML, missing fields, invalid data, env parsing, moderator IDs)
  - [ ] `cargo build` → compiles with zero errors

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Project compiles successfully
    Tool: Bash
    Preconditions: Rust toolchain installed
    Steps:
      1. Run: cargo build 2>&1
      2. Assert: exit code 0
      3. Assert: output does not contain "error["
    Expected Result: Clean compilation
    Evidence: Build output captured

  Scenario: Config loads valid TOML + env vars
    Tool: Bash
    Preconditions: docker-compose.dev.yml postgres running
    Steps:
      1. Run: cargo test config_loads_valid -- --nocapture 2>&1
      2. Assert: exit code 0
      3. Assert: output contains "test ... ok"
    Expected Result: Config parsing tests pass
    Evidence: Test output captured

  Scenario: Config rejects invalid TOML
    Tool: Bash
    Steps:
      1. Run: cargo test config_rejects_invalid -- --nocapture 2>&1
      2. Assert: exit code 0
      3. Assert: test checks for expected error variants
    Expected Result: Validation catches bad config
    Evidence: Test output captured

  Scenario: Dev database starts via docker-compose
    Tool: Bash
    Steps:
      1. Run: docker compose -f docker-compose.dev.yml up -d 2>&1
      2. Run: sleep 5
      3. Run: docker compose -f docker-compose.dev.yml ps --format json
      4. Assert: postgres service status is "running" or "healthy"
      5. Run: docker compose -f docker-compose.dev.yml down 2>&1
    Expected Result: PostgreSQL starts and becomes healthy
    Evidence: docker compose ps output captured
  ```

  **Commit**: YES
  - Message: `feat: project scaffolding with config system and test infrastructure`
  - Files: `Cargo.toml`, `src/**`, `docker-compose.dev.yml`, `.env.example`, `.env.test`, `config.example.toml`, `migrations/` (empty dir)
  - Pre-commit: `cargo build && cargo test`

---

- [ ] 2. Database Schema + Migrations

  **What to do**:

  **2a. Create all migration files** (8 tables + indexes + constraints):
  - `migrations/001_create_communities.sql` — communities table with unique index on `telegram_chat_id`
  - `migrations/002_create_community_questions.sql` — community_questions with FK to communities, unique index on `(community_id, position)`
  - `migrations/003_create_applicants.sql` — applicants with unique index on `telegram_user_id`
  - `migrations/004_create_join_requests.sql` — join_requests with FK to communities + applicants, status enum, indexes on `(community_id, status)` and `applicant_id`, unique constraint preventing duplicate active requests per applicant per community
  - `migrations/005_create_join_request_answers.sql` — join_request_answers with FK to join_requests + community_questions
  - `migrations/006_create_moderation_actions.sql` — moderation_actions with FK to join_requests, action_type enum
  - `migrations/007_create_blacklist_entries.sql` — blacklist_entries with scope_type enum, indexes
  - `migrations/008_create_applicant_sessions.sql` — applicant_sessions with FK to join_requests, state enum

  **2b. Status/enum types** (use PostgreSQL CHECK constraints or custom types):
  - `join_request_status`: `pending_contact`, `questionnaire_in_progress`, `submitted`, `approved`, `rejected`, `banned`, `expired`, `cancelled`
  - `moderation_action_type`: `approved`, `rejected`, `banned`
  - `blacklist_scope_type`: `global`, `community`
  - `session_state`: `awaiting_answer`, `completed`, `expired`, `cancelled`

  **2c. Key constraints to implement**:
  - All FKs with `ON DELETE CASCADE` where appropriate (e.g., answers cascade with join_request)
  - Unique constraint: `(applicant_id, community_id)` WHERE `status NOT IN ('approved', 'rejected', 'banned', 'expired', 'cancelled')` — prevents duplicate active join requests
  - `created_at` defaults to `NOW()`, `updated_at` auto-updated via trigger or application logic
  - All `telegram_*_id` fields as `BIGINT` (Telegram IDs can exceed i32 range)

  **2d. Config sync mechanism**:
  - Implement `sync_config_to_db()` function that runs at startup
  - Upsert communities by `telegram_chat_id`
  - Upsert questions by `(community_id, question_key)`
  - Deactivate questions in DB that are no longer in TOML (set `is_active = false`)
  - This goes in `src/db/mod.rs` or `src/db/sync.rs`

  **2e. Offline query metadata**:
  - Run `cargo sqlx prepare` to generate `.sqlx/` directory
  - Add `.sqlx/` to git tracking (needed for Docker builds)

  **TDD**:
  - **RED**: Write `#[sqlx::test]` tests — migrations apply cleanly, config sync upserts communities, config sync upserts questions, duplicate active join request constraint works, status enum values are valid
  - **GREEN**: Create migration files + implement `sync_config_to_db()`
  - **REFACTOR**: Ensure migration files are clean, indexes are correct

  **Must NOT do**:
  - Do NOT use an ORM — raw sqlx queries
  - Do NOT create migration runner scripts — use `sqlx::migrate!()` embedded at compile time
  - Do NOT add seed data in migrations — use config sync for community data
  - Do NOT make `telegram_user_chat_id` a permanent column in applicants (it's ephemeral per join request)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Database schema design + migration creation, standard backend work
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (solo)
  - **Blocks**: Task 3
  - **Blocked By**: Task 1

  **References**:
  - **User spec Section 10**: Complete data model with all fields for all 8 entities
  - **User spec Section 11**: Request status model with transitions
  - **User spec Section 26**: SQL-level constraints and indexes
  - **User spec Section 16**: Database requirements — idempotency, migrations
  - **Research finding (sqlx)**: Use `sqlx::migrate!("./migrations")` for embedded migrations; `query_as!` needs `.sqlx/` for offline mode

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test --test db` → ≥5 tests pass (migrations apply, config sync, constraints)
  - [ ] `cargo sqlx prepare --check` → exits 0 (offline metadata is up to date)

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Migrations apply cleanly to fresh database
    Tool: Bash
    Preconditions: docker-compose.dev.yml postgres running
    Steps:
      1. Run: DATABASE_URL=postgres://verifier_bot:verifier_bot@localhost:5433/verifier_bot_test cargo sqlx migrate run 2>&1
      2. Assert: exit code 0
      3. Assert: output contains "applied" for each migration
    Expected Result: All 8 migrations apply successfully
    Evidence: Migration output captured

  Scenario: Migrations are idempotent (re-running is safe)
    Tool: Bash
    Preconditions: Migrations already applied
    Steps:
      1. Run: DATABASE_URL=... cargo sqlx migrate run 2>&1
      2. Assert: exit code 0
      3. Assert: no errors (already applied migrations are skipped)
    Expected Result: Re-run is safe
    Evidence: Output captured

  Scenario: Config sync upserts communities
    Tool: Bash
    Steps:
      1. Run: cargo test config_sync_upserts_communities -- --nocapture 2>&1
      2. Assert: test passes
    Expected Result: TOML communities are synced to DB
    Evidence: Test output

  Scenario: Duplicate active join request is rejected by constraint
    Tool: Bash
    Steps:
      1. Run: cargo test duplicate_active_join_request_rejected -- --nocapture 2>&1
      2. Assert: test passes, sqlx returns unique violation error
    Expected Result: DB prevents duplicate active requests
    Evidence: Test output
  ```

  **Commit**: YES
  - Message: `feat: database schema with 8 tables, migrations, and config sync`
  - Files: `migrations/*.sql`, `src/db/mod.rs`, `src/db/sync.rs`, `.sqlx/`
  - Pre-commit: `cargo test --all`

---

- [ ] 3. Domain Models + Repository Layer

  **What to do**:

  **3a. Domain models** (`src/domain/`):
  - `src/domain/mod.rs` — module exports
  - `src/domain/community.rs` — `Community` struct (maps to `communities` table), `CommunityQuestion` struct
  - `src/domain/applicant.rs` — `Applicant` struct
  - `src/domain/join_request.rs` — `JoinRequest` struct with `JoinRequestStatus` enum, status transition validation method
  - `src/domain/answer.rs` — `JoinRequestAnswer` struct
  - `src/domain/moderation.rs` — `ModerationAction` struct with `ActionType` enum
  - `src/domain/blacklist.rs` — `BlacklistEntry` struct with `ScopeType` enum
  - `src/domain/session.rs` — `ApplicantSession` struct with `SessionState` enum
  - All structs derive `sqlx::FromRow`, `serde::Serialize`, `serde::Deserialize`, `Debug`, `Clone`
  - `JoinRequestStatus` enum must implement valid transition checking:
    - `pending_contact` → `questionnaire_in_progress`
    - `questionnaire_in_progress` → `submitted`
    - `submitted` → `approved` | `rejected` | `banned`
    - Any active status → `expired` | `cancelled`

  **3b. Error types** (`src/error.rs`):
  - Define `AppError` enum using `thiserror`:
    - `DatabaseError(sqlx::Error)`
    - `TelegramError(teloxide::RequestError)`
    - `ConfigError(String)`
    - `InvalidStateTransition { from: JoinRequestStatus, to: JoinRequestStatus }`
    - `NotFound(String)`
    - `Unauthorized(String)`
    - `AlreadyProcessed { join_request_id: i64, current_status: JoinRequestStatus }`
  - Implement `From<sqlx::Error>` and `From<teloxide::RequestError>` for `AppError`

  **3c. Repository layer** (`src/db/` or `src/repositories/`):
  - `CommunityRepo` — `find_by_telegram_chat_id()`, `find_active_questions(community_id)`
  - `ApplicantRepo` — `find_or_create_by_telegram_user_id()`, `update_profile()`
  - `JoinRequestRepo` — `create()`, `find_by_id()`, `find_active_for_applicant_in_community()`, `update_status()` (with optimistic locking via `updated_at`), `find_expired(timeout)`, `find_needing_reminder(timeout, reminder_offset)`
  - `AnswerRepo` — `create()`, `find_by_join_request_id()`
  - `ModerationActionRepo` — `create()`, `find_by_join_request_id()`
  - `BlacklistRepo` — `find_by_telegram_user_id(scope)`, `create()`
  - `SessionRepo` — `create()`, `find_active_by_join_request_id()`, `advance_question()`, `complete()`, `expire()`
  - Use `query_as!` macro for compile-time checked queries wherever possible
  - Use `sqlx::PgPool` as the connection parameter (not individual connections)
  - Implement `JoinRequestRepo::update_status()` with optimistic concurrency:
    ```sql
    UPDATE join_requests SET status = $2, updated_at = NOW()
    WHERE id = $1 AND status = $3 AND updated_at = $4
    RETURNING *
    ```
    Return error if 0 rows affected (concurrent modification)

  **TDD**:
  - **RED**: Write tests for each repo — CRUD operations, status transitions (valid and invalid), optimistic locking conflict, `find_or_create` idempotency, blacklist lookup
  - **GREEN**: Implement repo functions with sqlx queries
  - **REFACTOR**: Extract common query patterns, ensure error types are consistent

  **Must NOT do**:
  - Do NOT use an ORM (diesel, sea-orm) — raw sqlx
  - Do NOT make repositories generic/trait-based unless needed for testing — concrete structs are fine for MVP
  - Do NOT add business logic to repositories — they are pure data access
  - Do NOT use `query()` (runtime) where `query_as!` (compile-time) works

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Standard data access layer implementation
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (solo)
  - **Blocks**: Tasks 4, 5
  - **Blocked By**: Task 2

  **References**:
  - **User spec Section 10**: All entity fields — use these EXACTLY for struct definitions
  - **User spec Section 11**: Status transitions — implement as `JoinRequestStatus::can_transition_to(&self, target) -> bool`
  - **User spec Section 16.3**: Idempotency requirements
  - **User spec Section 12.2**: Double-processing protection — optimistic concurrency in `update_status()`
  - **Research finding (sqlx)**: Use `query_as!` with `FromRow` derive; use `fetch_optional` for nullable lookups, `fetch_one` for required, `fetch_all` for lists
  - **Research finding (teloxide repo pattern)**: DickGrowerBot `Repositories` struct that holds all repo instances with shared `PgPool`

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test --test repos` → ≥15 tests pass (CRUD for each repo, status transitions, optimistic locking)
  - [ ] `cargo build` → zero compilation errors with all `query_as!` macros

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Status transition validation
    Tool: Bash
    Steps:
      1. Run: cargo test status_transitions -- --nocapture 2>&1
      2. Assert: exit code 0
      3. Assert: valid transitions (pending_contact → questionnaire_in_progress) succeed
      4. Assert: invalid transitions (submitted → pending_contact) return InvalidStateTransition error
    Expected Result: State machine enforced correctly
    Evidence: Test output

  Scenario: Optimistic locking prevents double-processing
    Tool: Bash
    Steps:
      1. Run: cargo test optimistic_locking_conflict -- --nocapture 2>&1
      2. Assert: test creates join_request, updates status with stale updated_at, gets AlreadyProcessed error
    Expected Result: Concurrent modification detected
    Evidence: Test output

  Scenario: find_or_create_applicant is idempotent
    Tool: Bash
    Steps:
      1. Run: cargo test find_or_create_idempotent -- --nocapture 2>&1
      2. Assert: calling twice with same telegram_user_id returns same applicant ID
    Expected Result: No duplicate applicants
    Evidence: Test output
  ```

  **Commit**: YES
  - Message: `feat: domain models, error types, and repository layer with optimistic locking`
  - Files: `src/domain/**`, `src/error.rs`, `src/db/**` (or `src/repositories/**`), `tests/repos.rs`
  - Pre-commit: `cargo test --all`

---

- [ ] 4. Bot Dispatcher + Join Request Handler

  **What to do**:

  **4a. Bot initialization** (`src/bot/mod.rs`):
  - Create `Bot` from `BOT_TOKEN` env var
  - Set up `dptree::entry()` dispatcher with three top-level branches:
    1. `Update::filter_chat_join_request().endpoint(handle_join_request)`
    2. `Update::filter_message().filter(is_private_chat).endpoint(handle_private_message)`
    3. `Update::filter_callback_query().endpoint(handle_callback_query)`
  - **CRITICAL**: Set custom `distribution_function` keyed by `user_id`:
    ```rust
    .distribution_function(|upd: &Update| {
        upd.from().map(|user| user.id.0)
    })
    ```
    This serializes all updates from the same user, preventing the race condition between `chat_join_request` (group chat key) and user's first answer (private chat key).
  - Inject dependencies via `dptree::deps![]`:
    - `sqlx::PgPool`
    - `Arc<AppConfig>` (the loaded config)
  - Set `allowed_updates` to `[AllowedUpdate::Message, AllowedUpdate::CallbackQuery, AllowedUpdate::ChatJoinRequest]`

  **4b. Join request handler** (`src/bot/handlers/join_request.rs`):
  - Receive `ChatJoinRequest` from teloxide
  - Step 1: Look up community by `join_request.chat.id` in DB → if not found, log warning and return
  - Step 2: Check blacklist for `join_request.from.id` → if blacklisted, decline and return
  - Step 3: Find or create `Applicant` record from `join_request.from`
  - Step 4: Check for existing active join request for this applicant+community → if exists, this is a duplicate Telegram update, skip
  - Step 5: Create new `JoinRequest` record with status `pending_contact`, store `user_chat_id`
  - Step 6: **IMMEDIATELY** send first message to applicant via `bot.send_message(join_request.user_chat_id, ...)`:
    ```
    Hi {first_name}! I saw your request to join {community_title}.
    Before a moderator reviews it, please answer a few quick questions.

    {first_question_text}
    ```
  - Step 7: Create `ApplicantSession` record with `current_question_position = 1`, state `awaiting_answer`
  - Step 8: Update join request status to `questionnaire_in_progress`
  - Step 9: Handle errors:
    - If `send_message` returns 403 (user blocked bot): mark join request as `cancelled`, log
    - If `send_message` returns other error: log error, keep status as `pending_contact` for retry
  - Log with structured fields: `join_request_id`, `community_id`, `telegram_user_id`

  **4c. `/start` command handler** (fallback):
  - Handle `/start` in private chat
  - Check if user has any active `pending_contact` join request
  - If yes: resume — send first question, create session, update status
  - If no active request: send generic message: "Hi! If you've requested to join a community, I'll message you with some questions."
  - This handles the case where the 5-minute `user_chat_id` window expired and the user opened the bot manually

  **TDD**:
  - **RED**: Write tests for:
    - `handle_join_request` with known community → creates JoinRequest + Session + sends message
    - `handle_join_request` with unknown community → logs warning, no DB changes
    - `handle_join_request` with blacklisted user → declines, no questionnaire
    - `handle_join_request` duplicate → idempotent, no new records
    - `/start` with pending join request → resumes questionnaire
    - `/start` without pending request → generic message
  - Use `teremock` or mock the `Bot` trait for testing (extract interface if needed)
  - **GREEN**: Implement handlers
  - **REFACTOR**: Extract community lookup + blacklist check into a service function

  **Must NOT do**:
  - Do NOT put business logic beyond "create session + send Q1" in this handler
  - Do NOT approve or decline the join request here — that happens only after moderator acts
  - Do NOT use teloxide's built-in dialogue system
  - Do NOT delay the first message — send it as the first async operation after DB lookups
  - Do NOT use `.enable_ctrlc_handler()` — graceful shutdown is Task 8

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Async bot handler with race condition prevention, teloxide dispatcher wiring, multiple error handling paths — requires deep understanding
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4 (first of sequential pair with Task 5)
  - **Blocks**: Task 5
  - **Blocked By**: Task 3

  **References**:
  - **User spec Section 3**: Telegram constraints — join request flow, messaging exception, 5-minute window
  - **User spec Section 7.1**: Applicant flow steps 1-5 (the rest is Task 5)
  - **User spec Section 13.1**: Messaging strategy — ask questions one-by-one
  - **User spec Section 23**: Recommended applicant message templates
  - **User spec Section 19**: Error handling — applicant cannot be contacted, duplicate join request
  - **Metis directive**: `distribution_function` keyed by `user_id` — MUST implement this
  - **Metis directive**: Handle 403 on `send_message` → mark session cancelled
  - **Metis directive**: Set `allowed_updates` explicitly
  - **Research finding (teloxide)**: `Update::filter_chat_join_request().endpoint(handler)` for registration; `ChatJoinRequest.user_chat_id` is `ChatId` for messaging; `bot.approve_chat_join_request(chat_id, from.id)` for approval
  - **Research finding (teloxide)**: `dptree::deps![pool, config]` for dependency injection; handlers receive deps as function parameters

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test --test join_request` → ≥6 tests pass (happy path, unknown community, blacklist, duplicate, /start resume, /start no request)
  - [ ] Distribution function is set in dispatcher builder code

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Join request handler creates session and sends question
    Tool: Bash
    Steps:
      1. Run: cargo test join_request_creates_session -- --nocapture 2>&1
      2. Assert: exit code 0
      3. Assert: test verifies JoinRequest record created with status questionnaire_in_progress
      4. Assert: test verifies ApplicantSession record created with position 1
      5. Assert: test verifies send_message was called with first question text
    Expected Result: Full join request flow works
    Evidence: Test output

  Scenario: Blacklisted user is auto-declined
    Tool: Bash
    Steps:
      1. Run: cargo test blacklisted_user_declined -- --nocapture 2>&1
      2. Assert: exit code 0
      3. Assert: test verifies decline_chat_join_request was called
      4. Assert: no JoinRequest record created
    Expected Result: Blacklist check works
    Evidence: Test output

  Scenario: Dispatcher compiles with distribution_function
    Tool: Bash
    Steps:
      1. Run: cargo build 2>&1
      2. Assert: exit code 0
      3. Run: grep -r "distribution_function" src/bot/mod.rs
      4. Assert: distribution_function is present in dispatcher setup
    Expected Result: Race condition prevention is wired
    Evidence: Build output + grep result
  ```

  **Commit**: YES
  - Message: `feat: bot dispatcher with join request handler, /start fallback, and blacklist check`
  - Files: `src/bot/mod.rs`, `src/bot/handlers/mod.rs`, `src/bot/handlers/join_request.rs`, `src/bot/handlers/start.rs`, `src/main.rs`
  - Pre-commit: `cargo test --all`

---

- [ ] 5. Questionnaire FSM + Answer Persistence

  **What to do**:

  **5a. Private message handler** (`src/bot/handlers/questionnaire.rs`):
  - Filter: only private chat messages (not group messages, not commands)
  - Step 1: Look up active `ApplicantSession` for this `chat_id` / `user_id`
    - If no active session: ignore message (or send "I don't have an active questionnaire for you")
  - Step 2: Get current question from `community_questions` based on `session.current_question_position` and `join_request.community_id`
  - Step 3: Validate answer:
    - Non-empty for required questions
    - Minimum length check (configurable, default ≥2 characters)
    - Anti-low-effort: reject single-character or common placeholder answers (".", "x", "test", "asdf")
    - On validation failure: send friendly retry message, do NOT advance position
  - Step 4: Store answer in `join_request_answers` table
  - Step 5: Check if more questions remain:
    - If YES: advance `session.current_question_position`, send next question text
    - If NO: mark session as `completed`, update join request status to `submitted`, proceed to moderator delivery (call service function from Task 6)
  - Step 6: On completion, send confirmation message:
    ```
    Thanks — your application has been submitted to the moderators.
    You'll be notified once a decision is made.
    ```

  **5b. Questionnaire service** (`src/services/questionnaire.rs`):
  - Extract business logic into testable service functions:
    - `validate_answer(question: &CommunityQuestion, answer: &str) -> Result<(), ValidationError>`
    - `process_answer(pool: &PgPool, session: &ApplicantSession, answer: &str) -> Result<QuestionnaireStep, AppError>`
    - `QuestionnaireStep` enum: `NextQuestion { question: CommunityQuestion }` | `Completed { join_request: JoinRequest }`
  - Keep handler thin — it calls service functions and formats Telegram messages

  **5c. Answer validation rules**:
  - Required field: reject empty / whitespace-only
  - Min length: ≥2 characters for required questions
  - Anti-low-effort blocklist: reject exact matches against `[".", "..", "x", "xx", "test", "asdf", "123", "aaa", "-", "no", "n/a"]` (case-insensitive)
  - On failure: respond with specific message:
    - Empty: "This question is required. Please provide an answer."
    - Too short: "Please provide a more detailed answer (at least a few words)."
    - Low-effort: "Please provide a genuine answer so moderators can review your application."

  **TDD**:
  - **RED**: Write tests for:
    - `validate_answer` — valid answer, empty, too short, low-effort, optional question accepts empty
    - `process_answer` — advances to next question, stores answer, completes questionnaire on last question
    - Full flow: 5 questions answered one by one → status becomes `submitted`
    - Out-of-order messages (answer when no session) → gracefully ignored
    - Validation failure → does NOT advance position, same question re-asked
  - **GREEN**: Implement service + handler
  - **REFACTOR**: Ensure validation rules are configurable / extensible

  **Must NOT do**:
  - Do NOT ask all questions at once — one-by-one only
  - Do NOT approve/decline the join request on questionnaire completion — only submit to moderators
  - Do NOT store invalid answers in the database
  - Do NOT allow editing previous answers in MVP (too complex)
  - Do NOT send moderator card here — just change status to `submitted` and call the moderator delivery service (implemented in Task 6)

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: State machine logic with validation, multi-step async flow, error handling paths
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4 (sequential after Task 4)
  - **Blocks**: Task 6
  - **Blocked By**: Task 4

  **References**:
  - **User spec Section 7.1**: Applicant flow steps 5-10
  - **User spec Section 8**: Questionnaire requirements — one-by-one, validation, per-community config
  - **User spec Section 13**: Applicant messaging logic — strategy, completion, inactivity
  - **User spec Section 23**: Message templates for first message, completion, expiry
  - **Task 3**: `SessionRepo`, `AnswerRepo`, `JoinRequestRepo` — use these for all DB operations
  - **Task 4**: The dispatcher and join request handler — questionnaire handler is registered as the `filter_message` branch

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test --test questionnaire` → ≥10 tests pass (validation rules, FSM advancement, completion, edge cases)
  - [ ] `cargo test --test integration_flow` → full 5-question flow test passes

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Full questionnaire flow (5 questions)
    Tool: Bash
    Steps:
      1. Run: cargo test full_questionnaire_flow -- --nocapture 2>&1
      2. Assert: exit code 0
      3. Assert: test simulates 5 answers, each advances position
      4. Assert: after 5th answer, join_request.status == "submitted"
      5. Assert: 5 answer records exist in join_request_answers
    Expected Result: Complete flow works end-to-end
    Evidence: Test output

  Scenario: Validation rejects low-effort answers
    Tool: Bash
    Steps:
      1. Run: cargo test validation_rejects_low_effort -- --nocapture 2>&1
      2. Assert: "." and "test" and "" are all rejected
      3. Assert: session position does NOT advance on rejection
    Expected Result: Anti-spam validation works
    Evidence: Test output

  Scenario: No active session → message ignored
    Tool: Bash
    Steps:
      1. Run: cargo test no_session_message_ignored -- --nocapture 2>&1
      2. Assert: handler returns Ok without error
      3. Assert: no database changes
    Expected Result: Graceful handling of unexpected messages
    Evidence: Test output
  ```

  **Commit**: YES
  - Message: `feat: questionnaire FSM with answer validation and persistence`
  - Files: `src/bot/handlers/questionnaire.rs`, `src/services/questionnaire.rs`, `src/services/mod.rs`, `tests/questionnaire.rs`
  - Pre-commit: `cargo test --all`

---

- [ ] 6. Moderator Card + Inline Actions + Audit Trail

  **What to do**:

  **6a. Moderator card rendering** (`src/services/moderator.rs`):
  - Create formatted message for moderator chat (HTML parse mode):
    ```
    <b>📋 New Join Request</b>
    <b>Community:</b> {community_title}
    <b>Applicant:</b> {first_name} {last_name}
    <b>Username:</b> @{username} (or "not set")
    <b>Telegram ID:</b> <code>{telegram_user_id}</code>
    <b>Requested at:</b> {telegram_join_request_date} UTC
    <b>Completed at:</b> {questionnaire_completed_at} UTC

    <b>📝 Answers</b>
    1. <b>{question_text}:</b> {answer_text}
    2. <b>{question_text}:</b> {answer_text}
    ...

    <b>Status:</b> Submitted
    <b>Request ID:</b> <code>{join_request_id}</code>
    ```
  - Create inline keyboard with callback buttons:
    - `InlineKeyboardButton::callback("✅ Approve", format!("a:{}", join_request_id))`
    - `InlineKeyboardButton::callback("❌ Reject", format!("r:{}", join_request_id))`
    - `InlineKeyboardButton::callback("🚫 Ban", format!("b:{}", join_request_id))`
  - Callback data format: `{action_char}:{join_request_id}` (compact, well within 64-byte limit)

  **6b. Moderator card delivery** (`src/services/moderator.rs`):
  - Send card to `DEFAULT_MODERATOR_CHAT_ID` (from config)
  - Store `moderator_message_chat_id` and `moderator_message_id` in `join_requests` table (needed for editing card after action)
  - Update `submitted_to_moderators_at` timestamp
  - Handle send failure: log error, keep status as `submitted` (card can be re-sent)
  - This function is called from Task 5's questionnaire completion flow

  **6c. Callback query handler** (`src/bot/handlers/callbacks.rs`):
  - Parse callback data: split by `:` → extract action type + join_request_id
  - Step 1: Verify moderator authorization — check `from.id` against `ALLOWED_MODERATOR_IDS`
    - If unauthorized: `bot.answer_callback_query(q.id).text("⚠️ You are not authorized to moderate.").show_alert(true).await`
  - Step 2: Load join request from DB by ID
    - If not found: answer with "Request not found"
  - Step 3: Check current status — must be `submitted`
    - If already processed: answer with "⚠️ Already processed: {status} by {moderator}" (the double-processing protection)
  - Step 4: Execute action with optimistic locking (use `JoinRequestRepo::update_status` from Task 3):
    - **Approve**: Update status to `approved`, set `approved_at`, call `bot.approve_chat_join_request(community_chat_id, applicant_user_id)`, send approval message to applicant: "Your request to join {community} has been approved! Welcome!"
    - **Reject**: Update status to `rejected`, set `rejected_at`, call `bot.decline_chat_join_request(community_chat_id, applicant_user_id)`, send rejection message to applicant: "Unfortunately, your request to join {community} was not approved."
    - **Ban**: Update status to `banned`, call `bot.decline_chat_join_request(...)`, create `BlacklistEntry`, send message to applicant: "Your request to join {community} was declined."
  - Step 5: Create `ModerationAction` audit record
  - Step 6: Edit the moderator card — remove buttons, append action result:
    ```
    {original_card_text}

    ✅ Approved by @{moderator_username} at {timestamp} UTC
    ```
    Use `bot.edit_message_text(chat_id, message_id, new_text)` + `bot.edit_message_reply_markup(chat_id, message_id, empty_keyboard)`
  - Step 7: `bot.answer_callback_query(q.id).await` (acknowledge button press)
  - Error handling:
    - Telegram approve/decline API failure (400 `HIDE_REQUESTER_MISSING`): The user retracted the join request or another admin already processed it. Log warning, update DB status, edit card to show "⚠️ Join request was already processed outside the bot"
    - Optimistic locking failure: Another moderator acted first. Show "⚠️ This request was just processed by another moderator"

  **TDD**:
  - **RED**: Write tests for:
    - Card rendering — correct format with all fields
    - Callback parsing — valid and invalid callback data
    - Moderator authorization — allowed and denied
    - Approve flow — status change, Telegram API call, audit record, card edit
    - Reject flow — same
    - Ban flow — same + blacklist entry created
    - Double-processing — second moderator gets "already processed" response
    - Telegram API failure on approve (HIDE_REQUESTER_MISSING) — handled gracefully
  - **GREEN**: Implement services + handler
  - **REFACTOR**: Extract callback parsing, card formatting into pure functions

  **Must NOT do**:
  - Do NOT put full business state in callback data — only action + join_request_id
  - Do NOT use UUIDs in callback data (too long)
  - Do NOT trust callback clicks without moderator verification
  - Do NOT leave buttons active after action — edit message to remove them
  - Do NOT call `banChatMember` for the "Ban" action — only decline + blacklist for MVP

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Callback handling with concurrency protection, multiple Telegram API calls, error handling for API failures, audit trail — complex interaction flow
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 5 (solo)
  - **Blocks**: Tasks 7, 8
  - **Blocked By**: Task 5

  **References**:
  - **User spec Section 7.2**: Moderator flow — all steps
  - **User spec Section 9**: Moderator card requirements — content, buttons, post-action update
  - **User spec Section 12**: Moderator permissions — authorization, double-processing protection
  - **User spec Section 15**: Callback data design — compact format, resolve from DB
  - **User spec Section 22**: Recommended moderator message format (exact template)
  - **User spec Section 19**: Error handling — moderator clicks already-processed, Telegram API failure
  - **Metis directive**: Handle 400 `HIDE_REQUESTER_MISSING` — request processed by human admin
  - **Metis directive**: Use integer PKs in callback data, format `a:<id>`, `r:<id>`, `b:<id>`
  - **Research finding (teloxide)**: `InlineKeyboardButton::callback(label, data)` for buttons; `bot.answer_callback_query(q.id)` to acknowledge; `bot.edit_message_text()` to update card
  - **Research finding (teloxide)**: DickGrowerBot `CallbackResult` pattern for handling callback responses

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test --test moderation` → ≥10 tests pass (card render, callback parse, auth, approve, reject, ban, double-process, API failure)
  - [ ] `cargo test --test audit` → audit records are created for every moderation action

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Approve flow creates audit record and edits card
    Tool: Bash
    Steps:
      1. Run: cargo test approve_flow_full -- --nocapture 2>&1
      2. Assert: join_request status becomes "approved"
      3. Assert: moderation_actions record created with action_type "approved"
      4. Assert: edit_message_text was called to update card
      5. Assert: approve_chat_join_request was called
    Expected Result: Full approve lifecycle works
    Evidence: Test output

  Scenario: Double-processing returns friendly error
    Tool: Bash
    Steps:
      1. Run: cargo test double_processing_rejected -- --nocapture 2>&1
      2. Assert: second approve attempt returns AlreadyProcessed error
      3. Assert: answer_callback_query includes "already processed" text
    Expected Result: Race condition handled
    Evidence: Test output

  Scenario: Unauthorized moderator is rejected
    Tool: Bash
    Steps:
      1. Run: cargo test unauthorized_moderator -- --nocapture 2>&1
      2. Assert: callback handler rejects non-allowed user
      3. Assert: no DB changes, no Telegram API calls
    Expected Result: Authorization enforced
    Evidence: Test output
  ```

  **Commit**: YES
  - Message: `feat: moderator card delivery, inline callback actions, and audit trail`
  - Files: `src/bot/handlers/callbacks.rs`, `src/services/moderator.rs`, `tests/moderation.rs`
  - Pre-commit: `cargo test --all`

---

- [ ] 7. Expiry System + Blacklist + Reminder

  **What to do**:

  **7a. Background expiry task** (`src/services/expiry.rs`):
  - Spawn a `tokio::spawn` background task that runs every 60 seconds (configurable)
  - Query for join requests where:
    - Status is `pending_contact` or `questionnaire_in_progress`
    - `created_at` is older than `APPLICATION_TIMEOUT_MINUTES`
  - For each expired request:
    - Update status to `expired`
    - Update session state to `expired`
    - Send expiry message to applicant (if possible):
      ```
      Your application to join {community_title} timed out because we didn't receive all answers in time.
      You can request access again if needed.
      ```
    - Decline the join request via Telegram API (cleanup)
    - Handle 403/other errors on message send gracefully (user may have blocked bot)

  **7b. Reminder system**:
  - In the same background task, also query for requests needing a reminder:
    - Status is `questionnaire_in_progress`
    - `created_at` is older than `APPLICATION_TIMEOUT_MINUTES - REMINDER_BEFORE_EXPIRY_MINUTES`
    - No reminder has been sent yet (track via a `reminder_sent_at` column or flag — add migration)
  - Send reminder message:
    ```
    Just a reminder — your application to join {community_title} is still pending.
    Please answer the remaining questions, or your application will expire soon.
    ```
  - Update `reminder_sent_at` timestamp to prevent duplicate reminders

  **7c. Additional migration**:
  - Add `migrations/009_add_reminder_sent_at.sql`:
    ```sql
    ALTER TABLE join_requests ADD COLUMN reminder_sent_at TIMESTAMPTZ;
    ```

  **7d. Blacklist auto-decline enhancement**:
  - The blacklist check in Task 4's join request handler already declines blacklisted users
  - In this task, ensure the blacklist service is complete:
    - `BlacklistService::is_blacklisted(pool, telegram_user_id, community_id) -> bool` — checks both global and community-scoped entries
    - Blacklist entries created by the "Ban" action in Task 6 already work
  - Add logging: when a blacklisted user is auto-declined, log with full context

  **TDD**:
  - **RED**: Write tests for:
    - Expiry detection — finds requests older than timeout
    - Expiry processing — updates status, sends message, declines
    - Reminder detection — finds requests in the reminder window
    - Reminder sent — message sent, `reminder_sent_at` updated
    - No duplicate reminders — already reminded requests are skipped
    - Blacklist check — global scope, community scope, no match
  - **GREEN**: Implement expiry service + reminder + blacklist service
  - **REFACTOR**: Extract timing logic into testable pure functions

  **Must NOT do**:
  - Do NOT send more than one reminder per application
  - Do NOT expire requests that are already in `submitted` status (moderator hasn't acted yet — that's a different timeout if needed)
  - Do NOT run the background task more frequently than every 30 seconds (unnecessary load)
  - Do NOT block the main dispatcher with expiry processing — use `tokio::spawn`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Background task + DB queries + message sending — standard async Rust work
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 6 (with Task 8)
  - **Blocks**: Tasks 9, 10
  - **Blocked By**: Task 6

  **References**:
  - **User spec Section 13.3**: Inactivity handling — configurable timeout, reminder before expiry
  - **User spec Section 11**: Status transitions — any active status → `expired`
  - **User spec Section 23**: Expiry message template
  - **User spec Section 20**: Logging — "request expired" as important event
  - **Task 3**: `JoinRequestRepo::find_expired()`, `JoinRequestRepo::find_needing_reminder()` — implement these queries
  - **Task 4**: Blacklist check in join request handler — ensure consistency with blacklist service here

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test --test expiry` → ≥6 tests pass (detection, processing, reminder, no-duplicate, blacklist scopes)

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Expired requests are detected and processed
    Tool: Bash
    Steps:
      1. Run: cargo test expiry_processing -- --nocapture 2>&1
      2. Assert: request older than timeout is marked "expired"
      3. Assert: expiry message send was attempted
      4. Assert: decline_chat_join_request was called
    Expected Result: Expiry lifecycle works
    Evidence: Test output

  Scenario: Reminder sent exactly once
    Tool: Bash
    Steps:
      1. Run: cargo test reminder_sent_once -- --nocapture 2>&1
      2. Assert: reminder message sent for qualifying request
      3. Assert: reminder_sent_at is updated
      4. Assert: running again does NOT send second reminder
    Expected Result: No duplicate reminders
    Evidence: Test output
  ```

  **Commit**: YES
  - Message: `feat: application expiry with reminder, blacklist auto-decline`
  - Files: `src/services/expiry.rs`, `migrations/009_add_reminder_sent_at.sql`, `tests/expiry.rs`
  - Pre-commit: `cargo test --all`

---

- [ ] 8. Webhook Support + Graceful Shutdown + Error Handling

  **What to do**:

  **8a. Webhook mode** (`src/bot/webhook.rs`):
  - When `USE_WEBHOOKS=true`:
    - Start an `axum` HTTP server on `SERVER_PORT` (default 8080)
    - Register webhook endpoint: `POST /webhook` — receives Telegram updates
    - Call `bot.set_webhook(PUBLIC_WEBHOOK_URL)` on startup
    - Parse incoming JSON as `Update`, feed into teloxide dispatcher
    - Add `/health` GET endpoint returning `{"status": "ok", "mode": "webhook"}`
  - When `USE_WEBHOOKS=false` (default):
    - Use teloxide long polling (`bot.delete_webhook().await` first to clear any stale webhook)
    - Add `/health` endpoint anyway (useful for Docker health checks): start minimal axum server alongside polling
  - The switching logic goes in `src/main.rs`:
    ```rust
    if config.use_webhooks {
        start_webhook_mode(bot, handler, deps, config).await
    } else {
        start_polling_mode(bot, handler, deps, config).await
    }
    ```

  **8b. Graceful shutdown** (`src/bot/shutdown.rs`):
  - **CRITICAL**: Do NOT use `.enable_ctrlc_handler()` — it only handles SIGINT
  - Use `ShutdownToken` from teloxide dispatcher:
    ```rust
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(deps)
        .distribution_function(|upd: &Update| upd.from().map(|u| u.id.0))
        .build();

    let shutdown_token = dispatcher.shutdown_token();

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT, shutting down...");
            shutdown_token.shutdown().expect("shutdown");
        }
        _ = sigterm_signal() => {
            tracing::info!("Received SIGTERM, shutting down...");
            shutdown_token.shutdown().expect("shutdown");
        }
        _ = dispatcher.dispatch() => {
            tracing::info!("Dispatcher stopped");
        }
    }
    ```
  - Implement `sigterm_signal()` using `tokio::signal::unix::signal(SignalKind::terminate())`
  - On shutdown: finish processing current updates, close DB pool, exit cleanly

  **8c. Custom error handler**:
  - Replace teloxide's default `LoggingErrorHandler` (which silently discards failed updates)
  - Implement custom handler that:
    - Logs the full error with structured context
    - For `chat_join_request` errors: log at ERROR level (losing a join request is critical)
    - For transient DB errors: consider retry (but teloxide doesn't natively support retry — log for now)
    - For Telegram API errors: log with error code and description
  - Register via `.error_handler(LoggingErrorHandler::with_custom_text("Bot error"))` or custom `ErrorHandler` impl

  **TDD**:
  - **RED**: Write tests for:
    - Health endpoint returns 200 with correct JSON
    - Webhook endpoint parses valid Update JSON
    - Webhook endpoint rejects invalid JSON with 400
    - Config correctly toggles webhook vs polling mode
    - Shutdown signal handling (unit test the signal logic)
  - **GREEN**: Implement webhook server, shutdown handler, error handler
  - **REFACTOR**: Ensure clean separation between webhook and polling code paths

  **Must NOT do**:
  - Do NOT use `.enable_ctrlc_handler()` — use `ShutdownToken` + manual signal handling
  - Do NOT require TLS in the bot itself — assume reverse proxy (nginx/caddy) handles TLS for webhook
  - Do NOT make webhook mode the default — polling is default
  - Do NOT swallow errors in the custom error handler — always log with full context

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: HTTP server setup + signal handling — well-documented patterns
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 6 (with Task 7)
  - **Blocks**: Tasks 9, 10
  - **Blocked By**: Task 6

  **References**:
  - **User spec Section 17.3**: Webhook-related env vars (USE_WEBHOOKS, PUBLIC_WEBHOOK_URL, SERVER_PORT)
  - **User spec Section 18.3**: Startup behavior — load config, connect DB, run migrations, start polling/webhook
  - **User spec Section 19**: Error handling requirements — all error cases listed
  - **Metis directive**: Use `ShutdownToken` + SIGTERM handler (Docker sends SIGTERM)
  - **Metis directive**: Custom error handler — default `LoggingErrorHandler` discards failed updates
  - **Metis directive**: Set `allowed_updates` explicitly
  - **Research finding (teloxide)**: `ShutdownToken` API for graceful shutdown; `Dispatcher::builder().build()` returns dispatcher with `.shutdown_token()`

  **Acceptance Criteria**:

  **TDD**:
  - [ ] `cargo test --test webhook` → ≥4 tests pass (health endpoint, webhook parse, invalid JSON, config toggle)

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Health endpoint responds in polling mode
    Tool: Bash
    Preconditions: Bot binary running with USE_WEBHOOKS=false
    Steps:
      1. Start binary in background via tmux
      2. Wait 3 seconds for startup
      3. Run: curl -sf http://localhost:8080/health
      4. Assert: response contains "status":"ok"
      5. Kill process
    Expected Result: Health check works in polling mode
    Evidence: curl response captured

  Scenario: Binary handles SIGTERM gracefully
    Tool: Bash (tmux)
    Steps:
      1. Start binary in tmux session
      2. Wait 2 seconds
      3. Send: kill -TERM <pid>
      4. Wait 3 seconds
      5. Assert: output contains "Received SIGTERM" or "shutting down"
      6. Assert: process exited cleanly (exit code 0)
    Expected Result: Docker-compatible shutdown
    Evidence: Terminal output captured
  ```

  **Commit**: YES
  - Message: `feat: webhook support, graceful SIGTERM shutdown, custom error handler`
  - Files: `src/bot/webhook.rs`, `src/bot/shutdown.rs`, `src/bot/mod.rs` (updated), `src/main.rs` (updated), `tests/webhook.rs`
  - Pre-commit: `cargo test --all`

---

- [ ] 9. Docker Production Build

  **What to do**:

  **9a. Production Dockerfile** (multi-stage with cargo-chef):
  ```dockerfile
  FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
  WORKDIR /app

  FROM chef AS planner
  COPY . .
  RUN cargo chef prepare --recipe-path recipe.json

  FROM chef AS builder
  COPY --from=planner /app/recipe.json recipe.json
  RUN cargo chef cook --release --recipe-path recipe.json
  COPY . .
  ENV SQLX_OFFLINE=true
  RUN cargo build --release --bin verifier-bot

  FROM debian:bookworm-slim AS runtime
  RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
  WORKDIR /app
  COPY --from=builder /app/target/release/verifier-bot /usr/local/bin/verifier-bot
  COPY --from=builder /app/migrations /app/migrations
  RUN useradd -m -u 1001 appuser
  USER appuser
  EXPOSE 8080
  CMD ["verifier-bot"]
  ```
  - **CRITICAL**: `SQLX_OFFLINE=true` must be set in builder stage (no DB available during Docker build)
  - `.sqlx/` directory must be in the build context (checked into git in Task 2)
  - Copy `migrations/` directory into runtime image (needed for `sqlx::migrate!()` — actually, `sqlx::migrate!()` embeds at compile time, so this may not be needed. Verify.)
  - **IMPORTANT**: Check the latest version of `lukemathwalker/cargo-chef` and the latest Rust version available. Do NOT blindly use `latest-rust-1` — specify the actual Rust version tag.

  **9b. Production docker-compose.yml**:
  ```yaml
  services:
    postgres:
      image: postgres:16-alpine
      restart: unless-stopped
      environment:
        POSTGRES_USER: ${POSTGRES_USER:-verifier_bot}
        POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?POSTGRES_PASSWORD required}
        POSTGRES_DB: ${POSTGRES_DB:-verifier_bot}
      volumes:
        - postgres_data:/var/lib/postgresql/data
      healthcheck:
        test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-verifier_bot}"]
        interval: 10s
        timeout: 5s
        retries: 5
        start_period: 10s
      networks:
        - bot_network

    bot:
      build:
        context: .
        dockerfile: Dockerfile
      restart: unless-stopped
      environment:
        BOT_TOKEN: ${BOT_TOKEN:?BOT_TOKEN required}
        DATABASE_URL: postgres://${POSTGRES_USER:-verifier_bot}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB:-verifier_bot}
        RUST_LOG: ${RUST_LOG:-info,verifier_bot=debug}
        DEFAULT_MODERATOR_CHAT_ID: ${DEFAULT_MODERATOR_CHAT_ID:?DEFAULT_MODERATOR_CHAT_ID required}
        ALLOWED_MODERATOR_IDS: ${ALLOWED_MODERATOR_IDS:-}
        APPLICATION_TIMEOUT_MINUTES: ${APPLICATION_TIMEOUT_MINUTES:-60}
        REMINDER_BEFORE_EXPIRY_MINUTES: ${REMINDER_BEFORE_EXPIRY_MINUTES:-15}
        USE_WEBHOOKS: ${USE_WEBHOOKS:-false}
        PUBLIC_WEBHOOK_URL: ${PUBLIC_WEBHOOK_URL:-}
        SERVER_PORT: ${SERVER_PORT:-8080}
        CONFIG_PATH: /app/config.toml
      volumes:
        - ./config.toml:/app/config.toml:ro
      depends_on:
        postgres:
          condition: service_healthy
      ports:
        - "${SERVER_PORT:-8080}:8080"
      networks:
        - bot_network

  volumes:
    postgres_data:

  networks:
    bot_network:
  ```

  **9c. .dockerignore**:
  ```
  target/
  .env
  .env.*
  !.env.example
  .git/
  .sisyphus/
  ```

  **TDD**:
  - No traditional unit tests for Docker — QA scenarios cover this

  **Must NOT do**:
  - Do NOT expose PostgreSQL port externally in production compose (no `ports:` on postgres)
  - Do NOT hardcode any secrets in Dockerfile or docker-compose.yml
  - Do NOT use `latest` tags for base images — pin versions
  - Do NOT skip the non-root user in the runtime image

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Dockerfile + docker-compose.yml are well-defined patterns, mostly config files
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 7 (with Task 10)
  - **Blocks**: None
  - **Blocked By**: Tasks 7, 8

  **References**:
  - **User spec Section 18**: Deployment requirements — Docker Compose, Dockerfile, startup behavior
  - **User spec Section 21**: Security — DB not exposed externally, env vars for secrets
  - **Research finding (Docker)**: cargo-chef pattern: chef → planner → builder → runtime
  - **Research finding (Docker)**: `debian:bookworm-slim` for runtime, `ca-certificates` + `libssl3` for HTTPS
  - **Task 2**: `.sqlx/` directory must be in build context for `SQLX_OFFLINE=true`
  - **Task 8**: Health endpoint available for Docker health checks

  **Acceptance Criteria**:

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Docker image builds successfully
    Tool: Bash
    Steps:
      1. Run: docker compose build bot 2>&1
      2. Assert: exit code 0
      3. Assert: output contains "Successfully built" or equivalent
      4. Run: docker images | grep verifier-bot
      5. Assert: image exists
    Expected Result: Multi-stage build completes
    Evidence: Build output captured

  Scenario: Docker Compose starts full stack
    Tool: Bash
    Preconditions: .env file with valid BOT_TOKEN (can be dummy for startup test)
    Steps:
      1. Create temporary .env with dummy BOT_TOKEN and required vars
      2. Run: docker compose up -d 2>&1
      3. Run: sleep 10
      4. Run: docker compose ps --format json 2>&1
      5. Assert: postgres status is "running" and healthy
      6. Assert: bot status is "running"
      7. Run: docker compose logs bot --tail 20 2>&1
      8. Assert: logs contain migration success or DB connection success
      9. Run: docker compose down -v 2>&1
    Expected Result: Full stack starts and connects
    Evidence: docker compose ps + logs captured

  Scenario: Health check works in Docker
    Tool: Bash
    Preconditions: Stack is running
    Steps:
      1. Run: curl -sf http://localhost:8080/health
      2. Assert: response contains "status":"ok"
    Expected Result: Health endpoint accessible
    Evidence: curl response
  ```

  **Commit**: YES
  - Message: `feat: production Dockerfile with cargo-chef and docker-compose`
  - Files: `Dockerfile`, `docker-compose.yml`, `.dockerignore`
  - Pre-commit: `docker compose build`

---

- [ ] 10. Documentation + Polish

  **What to do**:

  **10a. README.md**:
  - Project description and purpose
  - Prerequisites (Docker, Docker Compose, Rust toolchain for development)
  - Telegram setup instructions:
    1. Create bot via @BotFather
    2. Get bot token
    3. Create private moderator supergroup, add bot as admin
    4. Get moderator chat ID (use @getmyid_bot or similar)
    5. Add bot as admin to target communities with `can_invite_users` permission
    6. Enable "Approve New Members" in community settings
    7. Get community chat IDs
  - Bot permissions required: `can_invite_users` (for processing join requests)
  - Quick start with Docker Compose:
    ```bash
    cp .env.example .env
    cp config.example.toml config.toml
    # Edit .env and config.toml with your values
    docker compose up -d
    ```
  - Development setup:
    ```bash
    docker compose -f docker-compose.dev.yml up -d  # Start test DB
    cp .env.example .env
    cargo run
    ```
  - Running tests: `cargo test --all`
  - Configuration reference (all env vars + TOML structure)
  - Architecture overview (brief)
  - Troubleshooting common issues

  **10b. Finalize `.env.example`**:
  ```bash
  # Required
  BOT_TOKEN=your_bot_token_from_botfather
  DATABASE_URL=postgres://verifier_bot:verifier_bot@localhost:5432/verifier_bot
  DEFAULT_MODERATOR_CHAT_ID=-1001234567890
  ALLOWED_MODERATOR_IDS=123456789,987654321

  # Optional
  RUST_LOG=info,verifier_bot=debug
  APPLICATION_TIMEOUT_MINUTES=60
  REMINDER_BEFORE_EXPIRY_MINUTES=15
  USE_WEBHOOKS=false
  PUBLIC_WEBHOOK_URL=https://your-domain.com/webhook
  SERVER_PORT=8080
  CONFIG_PATH=config.toml
  ```

  **10c. Finalize `config.example.toml`** (with two example communities):
  - Community 1: "DeFi Amsterdam" with 5 default questions
  - Community 2: "Rust Developers" with 3 custom questions
  - Include comments explaining each field

  **10d. Code polish**:
  - Run `cargo clippy --all-targets` and fix all warnings
  - Run `cargo fmt` for consistent formatting
  - Ensure all public items have doc comments
  - Review all `unwrap()` calls — replace with proper error handling
  - Verify `SQLX_OFFLINE` metadata is up to date: `cargo sqlx prepare --check`

  **10e. Final integration verification**:
  - Run full test suite: `cargo test --all`
  - Build Docker image: `docker compose build`
  - Start full stack: `docker compose up -d`
  - Verify health endpoint: `curl localhost:8080/health`
  - Check logs for clean startup: `docker compose logs bot`

  **TDD**: N/A (documentation + polish task)

  **Must NOT do**:
  - Do NOT include real bot tokens or chat IDs in examples
  - Do NOT over-document — keep README focused and actionable
  - Do NOT add architecture diagrams for MVP (text descriptions are sufficient)

  **Recommended Agent Profile**:
  - **Category**: `writing`
    - Reason: Documentation-heavy task with some code polish
  - **Skills**: `[]`

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 7 (with Task 9)
  - **Blocks**: None
  - **Blocked By**: Tasks 7, 8

  **References**:
  - **User spec Section 29**: Expected deliverables — README, .env.example, bot permissions, seed/config
  - **User spec Section 3.4**: Fallback copy for group description (include in README)
  - **User spec Section 17.3**: All env vars documented
  - **User spec Section 25**: Acceptance criteria (use as final verification checklist)

  **Acceptance Criteria**:

  **Agent-Executed QA Scenarios**:

  ```
  Scenario: Clippy passes with no warnings
    Tool: Bash
    Steps:
      1. Run: cargo clippy --all-targets 2>&1
      2. Assert: exit code 0
      3. Assert: no "warning" lines in output
    Expected Result: Clean codebase
    Evidence: Clippy output

  Scenario: All tests pass
    Tool: Bash
    Steps:
      1. Run: docker compose -f docker-compose.dev.yml up -d
      2. Wait for healthy
      3. Run: cargo test --all 2>&1
      4. Assert: exit code 0
      5. Assert: no test failures
    Expected Result: Full suite green
    Evidence: Test output

  Scenario: SQLX offline metadata is current
    Tool: Bash
    Steps:
      1. Run: cargo sqlx prepare --check 2>&1
      2. Assert: exit code 0
    Expected Result: .sqlx/ is up to date
    Evidence: Command output

  Scenario: README has all required sections
    Tool: Bash
    Steps:
      1. Run: grep -c "## " README.md
      2. Assert: ≥5 sections exist
      3. Run: grep -q "BotFather" README.md
      4. Assert: Telegram setup instructions present
      5. Run: grep -q "docker compose" README.md
      6. Assert: Docker instructions present
    Expected Result: README is comprehensive
    Evidence: grep results
  ```

  **Commit**: YES
  - Message: `docs: README, config examples, code polish with clippy fixes`
  - Files: `README.md`, `.env.example` (updated), `config.example.toml` (updated), any clippy-fixed source files
  - Pre-commit: `cargo clippy --all-targets && cargo test --all`

---

## Commit Strategy

| After Task | Message | Key Files | Verification |
|------------|---------|-----------|--------------|
| 1 | `feat: project scaffolding with config system and test infrastructure` | Cargo.toml, src/config.rs, docker-compose.dev.yml | `cargo build && cargo test` |
| 2 | `feat: database schema with 8 tables, migrations, and config sync` | migrations/*.sql, src/db/*.rs, .sqlx/ | `cargo test --all` |
| 3 | `feat: domain models, error types, and repository layer` | src/domain/**, src/error.rs, src/db/** | `cargo test --all` |
| 4 | `feat: bot dispatcher with join request handler and /start fallback` | src/bot/**, src/main.rs | `cargo test --all` |
| 5 | `feat: questionnaire FSM with answer validation and persistence` | src/bot/handlers/questionnaire.rs, src/services/questionnaire.rs | `cargo test --all` |
| 6 | `feat: moderator card, inline callback actions, and audit trail` | src/bot/handlers/callbacks.rs, src/services/moderator.rs | `cargo test --all` |
| 7 | `feat: application expiry with reminder, blacklist auto-decline` | src/services/expiry.rs, migrations/009_*.sql | `cargo test --all` |
| 8 | `feat: webhook support, graceful SIGTERM shutdown, custom error handler` | src/bot/webhook.rs, src/bot/shutdown.rs | `cargo test --all` |
| 9 | `feat: production Dockerfile with cargo-chef and docker-compose` | Dockerfile, docker-compose.yml, .dockerignore | `docker compose build` |
| 10 | `docs: README, config examples, code polish` | README.md, *.example | `cargo clippy && cargo test --all` |

---

## Success Criteria

### Verification Commands
```bash
# All tests pass
cargo test --all

# Clippy clean
cargo clippy --all-targets

# SQLX offline metadata current
cargo sqlx prepare --check

# Docker builds
docker compose build

# Full stack starts
docker compose up -d && sleep 10 && docker compose ps

# Health check
curl -sf http://localhost:8080/health | grep -q '"status":"ok"'

# Clean shutdown
docker compose down
```

### Final Checklist (from user spec Section 25)
- [ ] Bot starts via Docker Compose
- [ ] PostgreSQL starts and migrations apply automatically
- [ ] Bot can process join requests for at least one configured Telegram community
- [ ] On join request, applicant receives private onboarding questions automatically
- [ ] Applicant answers are persisted in PostgreSQL
- [ ] After questionnaire completion, moderator chat receives formatted application card
- [ ] Moderator can approve a request using inline button
- [ ] Moderator can reject a request using inline button
- [ ] Processed applications cannot be processed twice
- [ ] Moderator action is recorded in audit trail
- [ ] Multi-community routing works for at least two communities
- [ ] Inactive/stale applications expire correctly (with reminder)
- [ ] Logs are sufficiently structured for debugging
- [ ] The app can be run locally by another developer using .env.example + Docker Compose
- [ ] Webhook mode works when USE_WEBHOOKS=true
- [ ] Blacklisted users are auto-declined on join request
- [ ] /start fallback works for users who didn't receive automatic message
