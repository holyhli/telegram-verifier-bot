# /stats Command — Applicant Analytics & Per-Question Timing

## TL;DR

> **Quick Summary**: Add a `/stats` command that lets moderators see per-question timing analytics, where applicants get stuck, validation retry counts, and overall progress — with inline keyboard navigation for community selection, period filtering, and pagination. Includes a new `question_events` table for precise timing + retry tracking, and event instrumentation in the questionnaire flow.
> 
> **Deliverables**:
> - New DB migration `011_create_question_events.sql` with future-proof events schema
> - `QuestionEvent` domain model + `QuestionEventRepo` repository
> - Event instrumentation at 3 points in the questionnaire flow (question_presented, validation_failed, answer_accepted)
> - `/stats` command handler with moderator access control
> - Callback handlers for community selection → period filtering → results with pagination
> - Stats analytics service with time-period queries
> - Message formatter for stats output with inline keyboards
> - New `edit_message_html_with_markup` method on `TelegramApi` trait
> - TDD tests for all new components + full integration test
> - Updated `.sqlx/` offline metadata
> 
> **Estimated Effort**: Large
> **Parallel Execution**: YES — 4 waves + final verification
> **Critical Path**: T1 (migration) → T4 (repo) → T7 (instrumentation) → T10 (integration test) → T12 (sqlx prepare)

---

## Context

### Original Request
User wants to track metrics on which question people get stuck on and how long they take to answer each question. A `/stats` command in private messaging with the bot should show all applicants in progress (for day/week/month/all time) with per-question timing — including for completed applicants.

### Interview Summary
**Key Discussions**:
- **Access control**: Uses existing `ALLOWED_MODERATOR_IDS` from config — any moderator can use /stats
- **UX flow**: `/stats` → community selection buttons → period buttons (Today/Week/Month/All) → results view
- **Two views**: (a) Currently active people showing where they are + how long on current question, (b) Period summary with per-person per-question timing breakdown
- **Overflow**: Paginate with Next/Prev buttons, 10 users per page
- **Data tracking**: New `question_events` table logging question_presented, validation_failed, answer_accepted events — precise timing + retry counts
- **Schema design**: Future-proof for potential dashboard/export later
- **Tests**: TDD (test-first) following existing `FakeTelegramApi` mock pattern
- **Community scope**: Per-community with community selection step (skip if single community)

**Research Findings**:
- Existing data already partially supports timing (consecutive `join_request_answers.created_at` timestamps)
- Bot uses teloxide dptree dispatcher, repository pattern, sqlx compile-time checked queries
- Callback data prefix routing: `lang:`, `a:`, `r:`, `b:` — need new `s:` prefix for stats
- `.sqlx/` offline metadata must be regenerated after adding new queries
- `TelegramApi` trait lacks edit-with-keyboard method — must be extended (blocks all callback navigation)
- 9 existing test files with `FakeTelegramApi` mock — all need updating when trait changes

### Metis Review
**Identified Gaps** (addressed):
- **TelegramApi trait gap**: No `edit_message_html_with_markup` method → added as prerequisite task T2
- **Callback data 64-byte limit**: Must use compact encoding → `s:{id}:{period}:{view}:{page}` format
- **Event instrumentation must be non-blocking**: Failure to log events must not break user's questionnaire flow → fire-and-forget pattern
- **`.sqlx/` regeneration**: New queries require `cargo sqlx prepare` → explicit task T12
- **Test file ripple**: Trait extension requires updating all FakeTelegramApi mocks → bundled in T2
- **Stale pagination**: Accepted as snapshot-based; if page beyond range, show last page
- **Single-community shortcut**: Auto-skip community selection if only 1 community configured
- **"Stuck" definition**: Show ALL currently active applicants with time since last activity on current question
- **Stats language**: English-only (moderator tool, not user-facing)

---

## Work Objectives

### Core Objective
Enable moderators to understand applicant behavior through a `/stats` command that reveals per-question timing, stuck points, retry counts, and completion patterns — scoped by community and time period, with navigable inline keyboard UI.

### Concrete Deliverables
- `migrations/011_create_question_events.sql` — Events tracking table
- `src/domain/question_event.rs` — Domain model and event type enum
- `src/db/question_event_repo.rs` — Repository for event CRUD + analytics queries
- `src/services/stats.rs` — Analytics service (query aggregation, timing computation)
- `src/services/stats_formatter.rs` — Message formatting + keyboard generation
- `src/bot/handlers/stats.rs` — `/stats` command handler + callback handlers
- Extended `TelegramApi` trait with `edit_message_html_with_markup` method
- Updated `FakeTelegramApi` in all test files
- Event instrumentation in `questionnaire.rs` and `language_selection.rs`
- `tests/stats_tests.rs` — Comprehensive TDD tests
- Updated `.sqlx/*.json` offline metadata files

### Definition of Done
- [ ] `cargo test --all` passes with 0 failures (all existing + new tests)
- [ ] `cargo sqlx prepare --check` passes (offline metadata up to date)
- [ ] `cargo build` succeeds (compile-time query verification)
- [ ] `/stats` command responds with community selection (or period selection if single community)
- [ ] Full navigation flow works: community → period → view → pagination
- [ ] Events are recorded during questionnaire flow without blocking users
- [ ] Unauthorized users get no response to /stats

### Must Have
- `/stats` command accessible to ALLOWED_MODERATOR_IDS in private DM
- Community selection via inline buttons (skipped for single community)
- Period filtering: Today / This Week / This Month / All Time
- Currently active view: list of in-progress applicants with current question + time on it
- Period summary view: per-person per-question timing breakdown
- Pagination with Next/Prev buttons (10 per page)
- `question_events` table tracking: question_presented, validation_failed, answer_accepted
- Non-blocking event instrumentation (errors logged, not propagated to user)
- Access control check on both /stats command AND every callback
- Compact callback data within 64-byte Telegram limit

### Must NOT Have (Guardrails)
- No CSV export, web dashboard, or Grafana/Prometheus integration
- No bilingual stats output (English-only moderator tool)
- No database transactions (codebase uses zero transactions — use atomic single-statement ops)
- No modification to existing callback routing (`a:`, `r:`, `b:`, `lang:` prefixes untouched)
- No changes to existing table schemas (question_events is purely additive)
- No stats navigation state stored in the database (encode in callback_data only)
- No over-abstraction of the analytics queries (keep them direct sqlx queries, no ORM layer)
- No scheduled reports or automated stats delivery
- No per-moderator analytics tracking
- No AI-slop: no excessive comments, no unnecessary abstractions, no generic variable names

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES (9 test files, FakeTelegramApi, integration tests with real PostgreSQL)
- **Automated tests**: TDD (test-first) — each task follows RED (failing test) → GREEN (impl) → REFACTOR
- **Framework**: `cargo test` with `#[sqlx::test(migrations = "./migrations")]`
- **Test run command**: `DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all`

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Database/Repo**: Use Bash (`cargo test`) — run tests, assert pass counts
- **Bot handlers**: Use `cargo test` with `FakeTelegramApi` assertions on captured messages/keyboards
- **Integration**: Use Bash (`cargo test --test stats_tests`) — full flow tests
- **Build verification**: Use Bash (`cargo build && cargo sqlx prepare --check`)

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation — 3 parallel, no dependencies):
├── Task 1: Migration + domain model for question_events [quick]
├── Task 2: Extend TelegramApi trait + update ALL FakeTelegramApi mocks [unspecified-high]
└── Task 3: Callback data types + compact format parser [quick]

Wave 2 (Core Logic — 3 parallel, depend on Wave 1):
├── Task 4: QuestionEventRepo with TDD (depends: T1) [unspecified-high]
├── Task 5: Stats analytics service with TDD (depends: T1) [deep]
└── Task 6: Stats message formatter + keyboard builder with TDD (depends: T3) [unspecified-high]

Wave 3 (Integration — 3 parallel, depend on Wave 2):
├── Task 7: Event instrumentation in questionnaire flow (depends: T4) [unspecified-high]
├── Task 8: /stats command handler with TDD (depends: T2, T5, T6) [deep]
└── Task 9: Stats callback handlers with TDD (depends: T2, T5, T6) [deep]

Wave 4 (Verification — 2 sequential):
├── Task 10: Full end-to-end integration test (depends: T7, T8, T9) [deep]
├── Task 11: Regression check — all existing tests pass (depends: T10) [quick]
└── Task 12: cargo sqlx prepare + build + commit .sqlx/ (depends: T11) [quick]

Wave FINAL (After ALL tasks — 4 parallel reviews):
├── Task F1: Plan compliance audit (oracle)
├── Task F2: Code quality review (unspecified-high)
├── Task F3: Real manual QA (unspecified-high)
└── Task F4: Scope fidelity check (deep)

Critical Path: T1 → T4 → T7 → T10 → T11 → T12 → F1-F4
Parallel Speedup: ~60% faster than sequential
Max Concurrent: 3 (Waves 1, 2, 3)
```

### Dependency Matrix

| Task | Blocked By | Blocks |
|------|-----------|--------|
| T1 | — | T4, T5 |
| T2 | — | T8, T9 |
| T3 | — | T6 |
| T4 | T1 | T7 |
| T5 | T1 | T8, T9 |
| T6 | T3 | T8, T9 |
| T7 | T4 | T10 |
| T8 | T2, T5, T6 | T10 |
| T9 | T2, T5, T6 | T10 |
| T10 | T7, T8, T9 | T11 |
| T11 | T10 | T12 |
| T12 | T11 | F1-F4 |

### Agent Dispatch Summary

- **Wave 1**: 3 tasks — T1 → `quick`, T2 → `unspecified-high`, T3 → `quick`
- **Wave 2**: 3 tasks — T4 → `unspecified-high`, T5 → `deep`, T6 → `unspecified-high`
- **Wave 3**: 3 tasks — T7 → `unspecified-high`, T8 → `deep`, T9 → `deep`
- **Wave 4**: 3 tasks — T10 → `deep`, T11 → `quick`, T12 → `quick`
- **FINAL**: 4 tasks — F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

- [ ] 1. DB Migration + Domain Model for `question_events`

  **What to do**:
  - Create `migrations/011_create_question_events.sql` with schema:
    ```sql
    CREATE TABLE question_events (
        id BIGSERIAL PRIMARY KEY,
        join_request_id BIGINT NOT NULL REFERENCES join_requests(id),
        community_question_id BIGINT NOT NULL REFERENCES community_questions(id),
        applicant_id BIGINT NOT NULL REFERENCES applicants(id),
        event_type TEXT NOT NULL CHECK (event_type IN ('question_presented', 'validation_failed', 'answer_accepted')),
        metadata JSONB,  -- future-proof: validation error type, answer length, etc.
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );
    CREATE INDEX idx_question_events_join_request ON question_events(join_request_id);
    CREATE INDEX idx_question_events_type_created ON question_events(event_type, created_at);
    CREATE INDEX idx_question_events_community_question ON question_events(community_question_id, created_at);
    ```
  - Create `src/domain/question_event.rs` with:
    - `QuestionEventType` enum: `QuestionPresented`, `ValidationFailed`, `AnswerAccepted` with sqlx Type derive (TEXT, snake_case) following the exact pattern from `src/domain/join_request.rs:14-23` (JoinRequestStatus enum)
    - `QuestionEvent` struct with fields: `id: i64`, `join_request_id: i64`, `community_question_id: i64`, `applicant_id: i64`, `event_type: QuestionEventType`, `metadata: Option<serde_json::Value>`, `created_at: DateTime<Utc>`
    - Derive: `Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow`
  - Register module in `src/domain/mod.rs` — add `pub mod question_event;` and re-export types
  - Write RED test first: basic struct construction and enum serialization test in `tests/stats_tests.rs`

  **Must NOT do**:
  - Do not modify any existing migration files
  - Do not add foreign keys that cascade delete (events are an audit log)
  - Do not use database transactions

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Single migration file + simple domain model, follows established patterns
  - **Skills**: []
    - No special skills needed — straightforward Rust + SQL

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 2, 3)
  - **Blocks**: Tasks 4, 5
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References**:
  - `src/domain/join_request.rs:14-23` — `JoinRequestStatus` enum with sqlx Type derive pattern (TEXT + snake_case)
  - `src/domain/join_request.rs:25-40` — `JoinRequest` struct with all field types and derives
  - `src/domain/moderation.rs` — `ActionType` enum with sqlx derive (simpler enum example)
  - `src/domain/mod.rs` — Module registration and re-export pattern

  **API/Type References**:
  - `src/domain/answer.rs` — `JoinRequestAnswer` struct (similar shape — join_request_id + question_id + created_at)

  **Migration References**:
  - `migrations/004_create_join_requests.sql` — Example of CHECK constraint on TEXT enum column
  - `migrations/005_create_join_request_answers.sql` — Example of REFERENCES + index pattern
  - `migrations/008_create_applicant_sessions.sql` — Most recent migration pattern to follow

  **WHY Each Reference Matters**:
  - `JoinRequestStatus` enum: Copy the EXACT derive pattern (`#[derive(Debug, Clone, ...)]`, `#[sqlx(type_name = "text", rename_all = "snake_case")]`) — getting this wrong causes runtime panics
  - Migration 004: Shows how CHECK constraints are written for TEXT enum columns in this project
  - `JoinRequestAnswer`: The `question_events` struct is similar shape — use as template

  **Acceptance Criteria**:

  - [ ] Migration file `migrations/011_create_question_events.sql` exists and is valid SQL
  - [ ] `QuestionEventType` enum compiles with correct sqlx derives
  - [ ] `QuestionEvent` struct compiles with sqlx::FromRow derive
  - [ ] Module registered in `src/domain/mod.rs` with re-exports
  - [ ] `cargo build` succeeds

  **QA Scenarios:**

  ```
  Scenario: Migration applies cleanly to database
    Tool: Bash
    Preconditions: PostgreSQL running with existing schema (dev compose up)
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo sqlx migrate run
      2. Verify table exists: psql -h localhost -U verifier -d verifier_bot -c "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'question_events' ORDER BY ordinal_position"
      3. Assert columns: id (bigint), join_request_id (bigint), community_question_id (bigint), applicant_id (bigint), event_type (text), metadata (jsonb), created_at (timestamp with time zone)
    Expected Result: All 7 columns exist with correct types, 3 indexes created
    Failure Indicators: Migration error, missing columns, wrong types
    Evidence: .sisyphus/evidence/task-1-migration-applies.txt

  Scenario: Domain model compiles and enum serializes correctly
    Tool: Bash
    Preconditions: Migration applied, src/domain/question_event.rs exists
    Steps:
      1. Run: cargo build 2>&1
      2. Assert: exit code 0, no errors related to question_event module
    Expected Result: Clean build with no warnings about question_event
    Failure Indicators: Compile errors about missing derives, wrong sqlx type annotations
    Evidence: .sisyphus/evidence/task-1-domain-compiles.txt
  ```

  **Commit**: YES (group: T1)
  - Message: `feat(db): add question_events migration and domain model`
  - Files: `migrations/011_create_question_events.sql`, `src/domain/question_event.rs`, `src/domain/mod.rs`
  - Pre-commit: `cargo build`

---

- [ ] 2. Extend `TelegramApi` Trait + Update ALL `FakeTelegramApi` Mocks

  **What to do**:
  - Add new method to `TelegramApi` trait in `src/bot/handlers/mod.rs`:
    ```rust
    async fn edit_message_html_with_markup(
        &self,
        chat_id: i64,
        message_id: i32,
        text: String,
        reply_markup: Option<Vec<Vec<(String, String)>>>,
    ) -> Result<(), RequestError>;
    ```
    Use `Option<Vec<Vec<(String, String)>>>` for the keyboard (matching the existing `send_message_with_inline_keyboard` signature pattern), not `InlineKeyboardMarkup` directly
  - Implement in `TeloxideApi` (the real implementation) — call `bot.edit_message_text(...)` + `.reply_markup(InlineKeyboardMarkup::new(...))` if Some, or just edit text if None
  - Update `FakeTelegramApi` in ALL test files to implement the new trait method:
    - Add `edited_messages_with_markup: Arc<Mutex<Vec<(i64, i32, String, Option<Vec<Vec<(String, String)>>>)>>>` field
    - Initialize in `new()` and any `with_*` constructors
    - Implement trait method to push captures to the vec
  - RED test: Write a test in `tests/stats_tests.rs` that calls `edit_message_html_with_markup` on FakeTelegramApi and asserts the capture

  **Must NOT do**:
  - Do NOT modify the existing `edit_message_html` method signature (would break 3 call sites)
  - Do NOT remove any existing trait methods
  - Do NOT modify any existing test assertions — only ADD the new field/method

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Touches many files (trait + 6+ test files), requires careful coordination to avoid breaking existing tests
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 3)
  - **Blocks**: Tasks 8, 9
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References**:
  - `src/bot/handlers/mod.rs:25-95` — `TelegramApi` trait definition with all existing methods (especially `edit_message_html` at ~line 60 and `send_message_with_inline_keyboard` at ~line 45 for keyboard parameter pattern)
  - `src/bot/handlers/mod.rs:97-200` — `TeloxideApi` implementation (especially `edit_message_html` impl for the teloxide API call pattern)
  - `src/bot/handlers/callbacks.rs:195-210` — How `edit_message_html` is called in practice (for understanding the calling pattern)

  **Test References**:
  - `tests/handler_tests.rs:15-59` — `FakeTelegramApi` struct definition showing all Arc<Mutex<Vec>> fields and constructor
  - `tests/moderation_tests.rs` — Another FakeTelegramApi implementation (may differ slightly)
  - `tests/questionnaire_tests.rs` — Another FakeTelegramApi implementation
  - `tests/language_selection_tests.rs` — Another FakeTelegramApi implementation
  - `tests/expiry_tests.rs` — Another FakeTelegramApi implementation

  **WHY Each Reference Matters**:
  - `TelegramApi` trait: The new method MUST match the exact async_trait pattern and error type (RequestError)
  - `TeloxideApi` impl: Shows how to call teloxide's `bot.edit_message_text()` — the new method extends this
  - `FakeTelegramApi` in handler_tests.rs: The CANONICAL mock pattern — all other test files follow this structure
  - Each test file's FakeTelegramApi: ALL must be updated or compilation fails project-wide

  **Acceptance Criteria**:

  - [ ] `TelegramApi` trait has `edit_message_html_with_markup` method
  - [ ] `TeloxideApi` implements the method correctly
  - [ ] ALL `FakeTelegramApi` implementations compile (every test file)
  - [ ] `cargo test --all` passes (no regressions from trait change)

  **QA Scenarios:**

  ```
  Scenario: Trait extension compiles and all existing tests pass
    Tool: Bash
    Preconditions: Trait method added, all FakeTelegramApi updated
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all 2>&1
      2. Assert: exit code 0, no test failures
      3. Verify no compile errors related to TelegramApi trait
    Expected Result: All existing tests pass, zero regressions
    Failure Indicators: "method `edit_message_html_with_markup` is not a member" errors, test failures in existing tests
    Evidence: .sisyphus/evidence/task-2-trait-extension-tests.txt

  Scenario: FakeTelegramApi captures edit-with-markup calls
    Tool: Bash
    Preconditions: FakeTelegramApi updated with new field
    Steps:
      1. Write test that creates FakeTelegramApi, calls edit_message_html_with_markup with (123, 456, "text", Some(keyboard))
      2. Assert captured vec has 1 entry with matching values
      3. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --test stats_tests test_fake_api_captures_edit_with_markup 2>&1
    Expected Result: Test passes, captured data matches input
    Failure Indicators: Empty capture vec, wrong field values
    Evidence: .sisyphus/evidence/task-2-fake-api-capture.txt
  ```

  **Commit**: YES (group: T2)
  - Message: `refactor(bot): extend TelegramApi trait with edit_message_html_with_markup`
  - Files: `src/bot/handlers/mod.rs`, `tests/handler_tests.rs`, `tests/moderation_tests.rs`, `tests/questionnaire_tests.rs`, `tests/language_selection_tests.rs`, `tests/expiry_tests.rs`, `tests/stats_tests.rs`
  - Pre-commit: `DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all`

---

- [ ] 3. Stats Callback Data Types + Compact Format Parser

  **What to do**:
  - Create initial `src/bot/handlers/stats.rs` file with callback data types and parser:
    - Define `StatsCallbackData` enum:
      ```rust
      pub enum StatsCallbackData {
          SelectCommunity { community_id: i64 },
          SelectPeriod { community_id: i64, period: StatsPeriod },
          Navigate { community_id: i64, period: StatsPeriod, view: StatsView, page: u32 },
      }
      ```
    - Define `StatsPeriod` enum: `Today`, `ThisWeek`, `ThisMonth`, `AllTime`
    - Define `StatsView` enum: `Active`, `Summary`
    - Implement `StatsCallbackData::encode() -> String` — compact format:
      - Community select: `sc:{community_id}` (e.g., `sc:42`)
      - Period select: `sp:{community_id}:{t|w|m|a}` (e.g., `sp:42:w`)
      - Navigate: `sn:{community_id}:{t|w|m|a}:{c|s}:{page}` (e.g., `sn:42:w:c:1`)
    - Implement `StatsCallbackData::parse(data: &str) -> Option<StatsCallbackData>` — parse back from compact format
    - Implement `StatsPeriod` methods: `to_char()`, `from_char()`, `start_date() -> DateTime<Utc>` (computes period start)
    - Implement `StatsView` methods: `to_char()`, `from_char()`
  - Register module in `src/bot/handlers/mod.rs` — add `pub mod stats;`
  - RED tests first: Write parser round-trip tests (encode → parse → assert equal) and edge cases (invalid input → None)

  **Must NOT do**:
  - Do not add handler functions yet (just types + parser)
  - Do not import anything from services/ (no dependencies on analytics layer)
  - Callback data must fit within 64 bytes — validate with assertion in tests

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Pure data types + string parser, no DB or API dependencies
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2)
  - **Blocks**: Task 6
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References**:
  - `src/bot/handlers/callbacks.rs:49-85` — Existing callback data parsing pattern (`a:`, `r:`, `b:` prefix matching)
  - `src/bot/handlers/language_selection.rs:26-40` — `lang:en` / `lang:uk` callback data parsing
  - `src/domain/join_request.rs:14-23` — Enum definition pattern with Display/FromStr

  **WHY Each Reference Matters**:
  - callbacks.rs parsing: Shows the exact pattern for extracting callback data from prefix + splitting on `:`
  - language_selection.rs: Simpler callback format example — good model for `sc:` and `sp:` formats

  **Acceptance Criteria**:

  - [ ] `StatsCallbackData::encode()` produces compact strings within 64 bytes
  - [ ] `StatsCallbackData::parse()` round-trips correctly for all variants
  - [ ] `StatsPeriod::start_date()` returns correct UTC timestamps
  - [ ] Invalid callback data returns `None` (no panics)
  - [ ] `cargo build` succeeds

  **QA Scenarios:**

  ```
  Scenario: Callback data round-trips correctly
    Tool: Bash
    Preconditions: stats.rs types and parser implemented
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --test stats_tests test_callback_data_roundtrip 2>&1
      2. Assert: Test passes — encode then parse returns original data for all 3 variants
    Expected Result: All round-trip assertions pass
    Failure Indicators: Parse returns None, wrong variant, wrong field values
    Evidence: .sisyphus/evidence/task-3-callback-roundtrip.txt

  Scenario: Callback data fits within 64-byte Telegram limit
    Tool: Bash
    Preconditions: encode() implemented
    Steps:
      1. Test with worst-case: community_id = 9999999999 (10 digits), period = all, view = summary, page = 999
      2. Assert: encoded string length <= 64 bytes
    Expected Result: Even worst-case callback data fits in 64 bytes
    Failure Indicators: Encoded string exceeds 64 bytes
    Evidence: .sisyphus/evidence/task-3-callback-size.txt
  ```

  **Commit**: YES (group: T3)
  - Message: `feat(bot): add stats callback data types and parser`
  - Files: `src/bot/handlers/stats.rs`, `src/bot/handlers/mod.rs`, `tests/stats_tests.rs`
  - Pre-commit: `cargo build`

- [ ] 4. `QuestionEventRepo` with TDD — Event CRUD + Query Methods

  **What to do**:
  - RED: Write failing tests in `tests/stats_tests.rs` for:
    - `QuestionEventRepo::create()` — insert event, verify all fields returned
    - `QuestionEventRepo::find_by_join_request_id()` — fetch events for a join request
    - `QuestionEventRepo::count_validation_failures()` — count `validation_failed` events per question for a join request
  - GREEN: Create `src/db/question_event_repo.rs` implementing:
    ```rust
    pub struct QuestionEventRepo;
    impl QuestionEventRepo {
        pub async fn create(pool: &PgPool, join_request_id: i64, community_question_id: i64, applicant_id: i64, event_type: QuestionEventType, metadata: Option<serde_json::Value>) -> Result<QuestionEvent, AppError>
        pub async fn find_by_join_request_id(pool: &PgPool, join_request_id: i64) -> Result<Vec<QuestionEvent>, AppError>
        pub async fn count_validation_failures(pool: &PgPool, join_request_id: i64) -> Result<Vec<(i64, i64)>, AppError>  // (question_id, count)
    }
    ```
  - Use `sqlx::query_as!` with `as "event_type: QuestionEventType"` enum casting
  - Register in `src/db/mod.rs` — add `pub mod question_event_repo;` and re-export `QuestionEventRepo`
  - REFACTOR: Clean up any duplication

  **Must NOT do**:
  - Do not use transactions
  - Do not add complex aggregation queries yet (that's T5's job)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Involves TDD cycle + sqlx compile-time checked queries + enum casting pattern
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5, 6)
  - **Blocks**: Task 7
  - **Blocked By**: Task 1 (needs migration + domain model)

  **References**:

  **Pattern References**:
  - `src/db/answer_repo.rs` — Closest repo pattern: `AnswerRepo::create()` with query_as! INSERT RETURNING, and `find_by_join_request_id()` with ORDER BY
  - `src/db/moderation_repo.rs` — Another repo pattern with enum type casting in queries
  - `src/db/join_request_repo.rs:42-55` — Complex query_as! with `as "status: JoinRequestStatus"` enum casting pattern (MUST follow this for event_type)
  - `src/db/mod.rs` — Module registration pattern (pub mod + pub use re-exports)

  **Test References**:
  - `tests/repo_tests.rs` — Repository test patterns with `#[sqlx::test(migrations = "./migrations")]` and seed helpers
  - `tests/handler_tests.rs:555-629` — Test data seeding pattern (create community, applicant, join_request, etc.)

  **WHY Each Reference Matters**:
  - `answer_repo.rs`: Almost identical shape to what we need — INSERT RETURNING with created_at, SELECT WHERE join_request_id
  - `join_request_repo.rs:42-55`: Shows the critical `as "status: JoinRequestStatus"` enum casting — without this exact pattern, sqlx compile-time check fails
  - `repo_tests.rs`: Shows how to seed test data with #[sqlx::test] macro which provides a fresh DB per test

  **Acceptance Criteria**:

  - [ ] `QuestionEventRepo::create()` stores event and returns it with correct types
  - [ ] `QuestionEventRepo::find_by_join_request_id()` returns events ordered by created_at
  - [ ] `QuestionEventRepo::count_validation_failures()` returns correct per-question counts
  - [ ] All tests pass: `cargo test --test stats_tests`

  **QA Scenarios:**

  ```
  Scenario: Create and retrieve question events
    Tool: Bash
    Preconditions: Migration applied, domain model exists
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --test stats_tests test_question_event_repo_create 2>&1
      2. Assert: Test creates event with type=question_presented, retrieves it, verifies all fields match
    Expected Result: Event created, retrieved, fields match
    Failure Indicators: sqlx type casting error, missing columns, wrong event_type value
    Evidence: .sisyphus/evidence/task-4-repo-create.txt

  Scenario: Count validation failures per question
    Tool: Bash
    Preconditions: Events table populated with test data (3 failures on Q1, 1 on Q2)
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --test stats_tests test_count_validation_failures 2>&1
      2. Assert: Returns [(q1_id, 3), (q2_id, 1)]
    Expected Result: Correct per-question failure counts
    Evidence: .sisyphus/evidence/task-4-validation-counts.txt
  ```

  **Commit**: YES (group: T4)
  - Message: `feat(db): add QuestionEventRepo with create and query methods`
  - Files: `src/db/question_event_repo.rs`, `src/db/mod.rs`, `tests/stats_tests.rs`
  - Pre-commit: `DATABASE_URL="..." cargo test --test stats_tests`

---

- [ ] 5. Stats Analytics Service with TDD — Query Aggregation + Timing

  **What to do**:
  - RED: Write failing tests for:
    - `get_active_applicants(pool, community_id)` — returns list of in-progress applicants with current question position, time on current question, and applicant name/username
    - `get_period_summary(pool, community_id, period_start)` — returns all applicants who started in period with status, per-question timing, retry counts
    - `compute_per_question_timing(events)` — computes time between consecutive question_presented → answer_accepted events
  - GREEN: Create `src/services/stats.rs` implementing:
    ```rust
    pub struct StatsService;
    impl StatsService {
        // Active applicants: JOIN join_requests + applicant_sessions + applicants
        // WHERE status = 'questionnaire_in_progress' AND community_id = $1
        pub async fn get_active_applicants(pool: &PgPool, community_id: i64) -> Result<Vec<ActiveApplicantInfo>, AppError>
        
        // Period summary: JOIN join_requests + applicants + question_events
        // WHERE community_id = $1 AND created_at >= $2
        pub async fn get_period_summary(pool: &PgPool, community_id: i64, period_start: DateTime<Utc>) -> Result<Vec<ApplicantSummary>, AppError>
        
        // Pure function: compute timing from events list
        pub fn compute_per_question_timing(events: &[QuestionEvent]) -> Vec<QuestionTiming>
    }
    ```
  - Define result structs: `ActiveApplicantInfo` (name, username, current_question_pos, total_questions, time_on_current, time_started), `ApplicantSummary` (name, username, status, per_question_timings, total_time, retry_count), `QuestionTiming` (question_key, position, duration, retries)
  - Use `sqlx::query_as::<_, CustomRow>(r#"..."#)` for complex JOIN queries (following the pattern in `services/questionnaire.rs:126-176`)
  - Register in `src/services/mod.rs`

  **Must NOT do**:
  - Do not use transactions
  - Do not return formatted messages — only raw data structs (formatting is T6's job)
  - Do not touch handler code

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Complex SQL JOINs with multiple tables, time-based aggregation, timing computation logic
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4, 6)
  - **Blocks**: Tasks 8, 9
  - **Blocked By**: Task 1 (needs migration for question_events table)

  **References**:

  **Pattern References**:
  - `src/services/questionnaire.rs:126-176` — Complex JOIN query with `query_as::<_, ActiveQuestionnaireRow>` pattern — MUST follow this exact approach for multi-table analytics queries
  - `src/services/questionnaire.rs:60-93` — Custom row struct with aliased column names — use this pattern for analytics result rows
  - `src/services/moderator.rs` — Service layer pattern (struct with static async methods)

  **API/Type References**:
  - `src/domain/join_request.rs:JoinRequestStatus` — Status enum values for filtering (QuestionnaireInProgress, Submitted, etc.)
  - `src/domain/session.rs:SessionState` — Session states for the active query
  - `src/domain/question_event.rs:QuestionEventType` — Event types for timing computation

  **Test References**:
  - `tests/questionnaire_tests.rs` — Test patterns for service-layer logic with seeded DB data
  - `tests/handler_tests.rs:555-629` — Seed data creation helpers

  **WHY Each Reference Matters**:
  - `questionnaire.rs:126-176`: The EXACT pattern for complex multi-table JOINs with custom row structs and aliased columns. New analytics queries will be structurally similar.
  - `JoinRequestStatus` enum: Must filter on correct status values for active vs completed applicants
  - Seed data helpers: Tests need to create communities, questions, applicants, join requests, sessions, and events — must follow existing seeding pattern

  **Acceptance Criteria**:

  - [ ] `get_active_applicants()` returns correct list with timing info
  - [ ] `get_period_summary()` correctly filters by period and includes per-question data
  - [ ] `compute_per_question_timing()` correctly calculates durations from event timestamps
  - [ ] All tests pass: `cargo test --test stats_tests`

  **QA Scenarios:**

  ```
  Scenario: Get active applicants with timing
    Tool: Bash
    Preconditions: Seeded data — 2 applicants in questionnaire_in_progress, 1 completed
    Steps:
      1. Seed: community with 3 questions, 2 active applicants (at Q1 and Q3), 1 completed
      2. Run: DATABASE_URL="..." cargo test --test stats_tests test_get_active_applicants 2>&1
      3. Assert: Returns exactly 2 results, each with correct current_question_position and time_on_current > 0
    Expected Result: Only in-progress applicants returned with correct positions
    Failure Indicators: Returns 3 (includes completed), wrong positions, zero timing
    Evidence: .sisyphus/evidence/task-5-active-applicants.txt

  Scenario: Period summary filters by time range
    Tool: Bash
    Preconditions: Seeded data — 3 applicants: 1 from today, 1 from 3 days ago, 1 from 2 weeks ago
    Steps:
      1. Call get_period_summary with period_start = 7 days ago
      2. Assert: Returns 2 applicants (today + 3 days ago), not the 2-week-old one
    Expected Result: Only applicants within period are returned
    Evidence: .sisyphus/evidence/task-5-period-filter.txt

  Scenario: Per-question timing computation
    Tool: Bash
    Preconditions: Events list with question_presented at T, answer_accepted at T+5min, next question_presented at T+5min, answer_accepted at T+12min
    Steps:
      1. Call compute_per_question_timing with events
      2. Assert: Q1 duration = 5min, Q2 duration = 7min
    Expected Result: Correct durations computed from event pairs
    Evidence: .sisyphus/evidence/task-5-timing-computation.txt
  ```

  **Commit**: YES (group: T5)
  - Message: `feat(services): add stats analytics service with period queries`
  - Files: `src/services/stats.rs`, `src/services/mod.rs`, `tests/stats_tests.rs`
  - Pre-commit: `DATABASE_URL="..." cargo test --test stats_tests`

---

- [ ] 6. Stats Message Formatter + Keyboard Builder with TDD

  **What to do**:
  - RED: Write failing tests for:
    - `format_community_selection(communities)` — returns text + keyboard with community buttons
    - `format_period_selection(community_title, community_id)` — returns text + keyboard with period buttons
    - `format_active_view(community_title, period, applicants, page, total_pages)` — returns formatted text + pagination keyboard
    - `format_summary_view(community_title, period, summaries, page, total_pages)` — returns formatted text + pagination keyboard
  - GREEN: Create `src/services/stats_formatter.rs` implementing:
    ```rust
    pub struct StatsFormatter;
    impl StatsFormatter {
        pub fn format_community_selection(communities: &[(i64, String)]) -> (String, Vec<Vec<(String, String)>>)
        pub fn format_period_selection(community_title: &str, community_id: i64) -> (String, Vec<Vec<(String, String)>>)
        pub fn format_active_view(community_title: &str, period_label: &str, applicants: &[ActiveApplicantInfo], page: u32, total_pages: u32) -> (String, Vec<Vec<(String, String)>>)
        pub fn format_summary_view(community_title: &str, period_label: &str, summaries: &[ApplicantSummary], page: u32, total_pages: u32) -> (String, Vec<Vec<(String, String)>>)
    }
    ```
  - Message format for active view (each applicant entry):
    ```
    📊 [Community] — Active (Today)
    
    🔄 2 applicants in progress:
    
    1. John (@johndoe)
       📍 Question 3/5 — "How did you hear about us?"
       ⏱ 23m on this question | Started 45m ago
       🔄 2 retries on current question
    
    2. Alice
       📍 Question 1/5 — "What is your name?"
       ⏱ 4m on this question | Started 4m ago
    ```
  - Message format for summary view:
    ```
    📊 [Community] — Summary (This Week)
    
    1. John (@johndoe) — ✅ Approved
       Q1 (Name): 1m 12s
       Q2 (Occupation): 15m 30s ⚠️
       Q3 (Referral): 45s
       Total: 17m 27s | Retries: 2
    ```
  - Keyboard builder: generate callback data using `StatsCallbackData::encode()` from T3
  - Pagination keyboard: [◀ Prev] [Active | Summary] [Next ▶] — encode page+1 / page-1 in callback data
  - Use HTML formatting (the bot uses `send_message_html`)
  - Register in `src/services/mod.rs`

  **Must NOT do**:
  - Do not query the database (formatter is pure — takes data, returns formatted text)
  - Do not handle Telegram API calls (that's T8/T9's job)
  - Messages must stay within 4096 characters — truncate with "... and N more" if needed

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Complex message formatting with HTML, keyboard generation, pagination logic
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4, 5)
  - **Blocks**: Tasks 8, 9
  - **Blocked By**: Task 3 (needs StatsCallbackData types for keyboard encoding)

  **References**:

  **Pattern References**:
  - `src/services/moderator.rs:83-124` — Message formatting pattern with HTML tags + inline keyboard building. This is the CLOSEST existing pattern to what we need.
  - `src/bot/handlers/questionnaire.rs:84-87` — Keyboard construction: `vec![vec![(label, callback_data), ...]]`
  - `src/messages.rs` — Message text patterns and formatting conventions

  **API/Type References**:
  - `src/bot/handlers/stats.rs:StatsCallbackData` — Callback data encoding (from T3)
  - `src/services/stats.rs:ActiveApplicantInfo, ApplicantSummary` — Input data structs (from T5)

  **WHY Each Reference Matters**:
  - `moderator.rs:83-124`: Shows EXACTLY how the bot formats rich messages with HTML + inline keyboards — the stats formatter will follow this pattern
  - Keyboard construction in questionnaire.rs: The `Vec<Vec<(String, String)>>` type for inline keyboards — must match

  **Acceptance Criteria**:

  - [ ] Community selection shows one button per community
  - [ ] Period selection shows 4 buttons (Today/Week/Month/All)
  - [ ] Active view shows per-applicant entries with timing and question info
  - [ ] Summary view shows per-applicant per-question timing
  - [ ] Pagination keyboard shows when total_pages > 1
  - [ ] All formatted messages fit within 4096 characters
  - [ ] All tests pass: `cargo test --test stats_tests`

  **QA Scenarios:**

  ```
  Scenario: Community selection format
    Tool: Bash
    Preconditions: Formatter implemented
    Steps:
      1. Call format_community_selection with 2 communities: [(1, "DeFi Amsterdam"), (2, "Crypto Berlin")]
      2. Assert text contains "Select a community"
      3. Assert keyboard has 2 buttons with callback data "sc:1" and "sc:2"
    Expected Result: Correct text + keyboard with encoded callback data
    Evidence: .sisyphus/evidence/task-6-community-selection.txt

  Scenario: Active view pagination
    Tool: Bash
    Preconditions: Formatter implemented
    Steps:
      1. Call format_active_view with 25 applicants, page=1, total_pages=3
      2. Assert: Text shows first 10 applicants
      3. Assert: Keyboard has [Active | Summary] toggle + [Next ▶] button (no Prev on page 1)
    Expected Result: First page shows 10 entries, correct nav buttons
    Evidence: .sisyphus/evidence/task-6-pagination.txt

  Scenario: Message length within Telegram limit
    Tool: Bash
    Preconditions: Formatter implemented
    Steps:
      1. Call format_active_view with 10 applicants each having 5-question timing data
      2. Assert: formatted text length <= 4096 characters
    Expected Result: Message fits within limit, truncated if necessary with "... and N more"
    Evidence: .sisyphus/evidence/task-6-message-length.txt
  ```

  **Commit**: YES (group: T6)
  - Message: `feat(services): add stats message formatter and keyboard builder`
  - Files: `src/services/stats_formatter.rs`, `src/services/mod.rs`, `tests/stats_tests.rs`
  - Pre-commit: `cargo test --test stats_tests`

- [ ] 7. Event Instrumentation in Questionnaire Flow

  **What to do**:
  - Instrument 3 event tracking points in the questionnaire flow. Each insertion must be **non-blocking** — use `if let Err(e) = ... { tracing::error!(...) }` pattern, never propagate event errors to the user.
  - **Point 1: question_presented** (when a question is sent to the user)
    - In `src/bot/handlers/questionnaire.rs` at the `NextQuestion` match arm (around line 116-119) — after `api.send_message` succeeds, insert:
      ```rust
      if let Err(e) = QuestionEventRepo::create(pool, jr_id, question.id, applicant_id, QuestionEventType::QuestionPresented, None).await {
          tracing::error!(join_request_id = jr_id, error = %e, "failed to record question_presented event");
      }
      ```
    - In `src/bot/handlers/language_selection.rs` around line 87 — when the first question is sent after language selection, insert the same event
  - **Point 2: validation_failed** (when validation rejects an answer)
    - In `src/bot/handlers/questionnaire.rs` at the `ValidationFailed` match arm (around line 108-112) — after sending error message, insert:
      ```rust
      if let Err(e) = QuestionEventRepo::create(pool, jr_id, question_id, applicant_id, QuestionEventType::ValidationFailed, Some(json!({"reason": "..."}))  ).await {
          tracing::error!(...);
      }
      ```
  - **Point 3: answer_accepted** (when an answer passes validation and is stored)
    - In `src/services/questionnaire.rs` at line 233-239 — after `AnswerRepo::create` succeeds, insert event
  - Pass the pool reference that's already available at each instrumentation point
  - Will need to also pass `context.join_request.id`, `context.current_question.id`, and `context.join_request.applicant_id` which are already available from `ActiveQuestionnaireContext`

  **Must NOT do**:
  - Event failures must NEVER propagate to the user — always catch and log
  - Do not modify the flow logic (if/else, return paths, state transitions)
  - Do not add new function parameters to existing public functions if avoidable
  - Do not modify the existing test expectations (existing tests should still pass unchanged)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Touches 3 files at precise insertion points, requires understanding the async flow and error handling pattern
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 8, 9)
  - **Blocks**: Task 10
  - **Blocked By**: Task 4 (needs QuestionEventRepo)

  **References**:

  **Exact Insertion Points**:
  - `src/bot/handlers/questionnaire.rs:107-132` — The match arms where ValidationFailed (line 108) and NextQuestion (line 113) are handled — instrumentation goes AFTER the api.send_message calls
  - `src/bot/handlers/language_selection.rs:83-95` — Where first question is sent after language selection — instrument after the question is sent
  - `src/services/questionnaire.rs:233-239` — Where `AnswerRepo::create` is called — instrument after answer is stored

  **Pattern References**:
  - `src/services/expiry.rs:108-132` — Error handling pattern in background tasks — uses `if let Err(e)` with tracing::error
  - `src/bot/handlers/questionnaire.rs:78-83` — Existing tracing::info pattern with structured fields

  **API/Type References**:
  - `src/db/question_event_repo.rs:QuestionEventRepo::create()` — The method to call at each point (from T4)
  - `src/domain/question_event.rs:QuestionEventType` — The event type enum variants

  **WHY Each Reference Matters**:
  - Insertion points: MUST instrument at the EXACT lines specified — before these points, data isn't yet available; after these points, the flow has already moved on
  - Error handling pattern: The `if let Err(e)` + tracing pattern is standard in this codebase — MUST follow it for consistency
  - ActiveQuestionnaireContext: All needed IDs (join_request_id, question_id, applicant_id) are available from this struct which is already in scope

  **Acceptance Criteria**:

  - [ ] `question_presented` events recorded when questions are sent
  - [ ] `validation_failed` events recorded when validation fails
  - [ ] `answer_accepted` events recorded when answers are stored
  - [ ] Event failures do NOT break the questionnaire flow
  - [ ] All existing questionnaire tests still pass (zero regressions)

  **QA Scenarios:**

  ```
  Scenario: Events recorded during normal questionnaire flow
    Tool: Bash
    Preconditions: Full bot infrastructure set up, events table exists
    Steps:
      1. Run: DATABASE_URL="..." cargo test --test stats_tests test_events_recorded_during_questionnaire 2>&1
      2. Test simulates: question presented → validation failure → successful answer → next question
      3. Assert: 2 question_presented events, 1 validation_failed event, 1 answer_accepted event in DB
    Expected Result: All events correctly recorded with correct types and foreign keys
    Failure Indicators: Missing events, wrong event types, broken foreign key references
    Evidence: .sisyphus/evidence/task-7-events-recorded.txt

  Scenario: Event failure doesn't break questionnaire
    Tool: Bash
    Preconditions: QuestionEventRepo::create mocked/configured to return error
    Steps:
      1. Run test that processes a valid answer with event tracking returning Err
      2. Assert: Answer is still stored, next question is still sent, no error returned to user
    Expected Result: Questionnaire flow continues despite event recording failure
    Evidence: .sisyphus/evidence/task-7-non-blocking.txt

  Scenario: Existing questionnaire tests still pass
    Tool: Bash
    Preconditions: Instrumentation code added
    Steps:
      1. Run: DATABASE_URL="..." cargo test --test questionnaire_tests 2>&1
      2. Run: DATABASE_URL="..." cargo test --test handler_tests 2>&1
      3. Assert: All existing tests pass with 0 failures
    Expected Result: Zero regressions
    Evidence: .sisyphus/evidence/task-7-no-regression.txt
  ```

  **Commit**: YES (group: T7)
  - Message: `feat(bot): instrument question events in questionnaire flow`
  - Files: `src/bot/handlers/questionnaire.rs`, `src/bot/handlers/language_selection.rs`, `src/services/questionnaire.rs`
  - Pre-commit: `DATABASE_URL="..." cargo test --all`

---

- [ ] 8. `/stats` Command Handler with TDD

  **What to do**:
  - RED: Write failing tests for:
    - Authorized moderator sends /stats → receives community selection keyboard (multi-community)
    - Authorized moderator sends /stats → receives period selection directly (single community)
    - Unauthorized user sends /stats → no response
  - GREEN: Implement in `src/bot/handlers/stats.rs` (extending the file from T3):
    - `handle_stats_command(bot, msg, pool, config)` — entry point (registered in dispatcher)
    - `process_stats_command(api, pool, config, input)` — testable processor
    - Input struct: `StatsCommandInput { chat_id: i64, telegram_user_id: i64 }`
    - Logic:
      1. Check `config.allowed_moderator_ids.contains(&input.telegram_user_id)` — if not, return Ok(()) silently
      2. Load active communities from DB
      3. If 1 community: skip to period selection (call `StatsFormatter::format_period_selection`)
      4. If multiple: show community selection (call `StatsFormatter::format_community_selection`)
      5. Send message with inline keyboard via `api.send_message_with_inline_keyboard`
  - Register `/stats` command in bot dispatcher (`src/bot/mod.rs:schema()`):
    - Add new filter branch within the message handler for `/stats` in private chats
    - Follow the exact pattern of how `/start` is routed

  **Must NOT do**:
  - Do not implement callback handlers here (that's T9)
  - Do not modify the existing `/start` handler routing
  - Do not add /stats routing in group chats — private DM only

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Requires dispatcher schema modification + handler + access control + TDD
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 7, 9)
  - **Blocks**: Task 10
  - **Blocked By**: Tasks 2 (TelegramApi trait), 5 (analytics service), 6 (formatter)

  **References**:

  **Pattern References**:
  - `src/bot/handlers/start.rs:43-93` — Command handler pattern: extract input struct, call testable processor fn — EXACTLY this pattern for /stats
  - `src/bot/mod.rs:schema()` — Dispatcher schema with dptree filter chains — must add /stats branch here
  - `src/bot/handlers/callbacks.rs:49-60` — Access control check pattern: `config.allowed_moderator_ids.contains(&user_id)`

  **API/Type References**:
  - `src/services/stats_formatter.rs:StatsFormatter` — format_community_selection, format_period_selection (from T6)
  - `src/db/community_repo.rs:CommunityRepo` — Community query methods for loading active communities
  - `src/config.rs:Config` — `allowed_moderator_ids: Vec<i64>` field for access control

  **Test References**:
  - `tests/handler_tests.rs:564-629` — /start command test pattern with FakeTelegramApi

  **WHY Each Reference Matters**:
  - `start.rs:43-93`: The EXACT pattern to follow — `handle_*` wraps teloxide types, calls `process_*` with `&dyn TelegramApi`
  - `mod.rs:schema()`: The dptree configuration MUST be extended correctly or the command won't route
  - Access control pattern: The authorization check must happen INSIDE `process_stats_command` (not in the schema filter) so it can be tested

  **Acceptance Criteria**:

  - [ ] `/stats` from moderator in DM shows community selection (or period if single community)
  - [ ] `/stats` from non-moderator produces no response
  - [ ] `/stats` in group chat produces no response (only private DM)
  - [ ] Command registered in dispatcher schema
  - [ ] All tests pass: `cargo test --test stats_tests`

  **QA Scenarios:**

  ```
  Scenario: Moderator receives community selection
    Tool: Bash
    Preconditions: 2 communities configured, moderator user ID in allowed list
    Steps:
      1. Run: DATABASE_URL="..." cargo test --test stats_tests test_stats_command_multi_community 2>&1
      2. Assert: FakeTelegramApi.keyboards_sent has 1 entry with 2 community buttons
      3. Assert: Keyboard button callback data starts with "sc:"
    Expected Result: Community selection keyboard sent to moderator
    Evidence: .sisyphus/evidence/task-8-multi-community.txt

  Scenario: Single community skips to period selection
    Tool: Bash
    Preconditions: 1 community configured
    Steps:
      1. Run: DATABASE_URL="..." cargo test --test stats_tests test_stats_command_single_community 2>&1
      2. Assert: FakeTelegramApi.keyboards_sent has 1 entry with 4 period buttons (Today/Week/Month/All)
    Expected Result: Period selection keyboard sent directly (no community step)
    Evidence: .sisyphus/evidence/task-8-single-community.txt

  Scenario: Unauthorized user gets no response
    Tool: Bash
    Preconditions: User ID NOT in allowed_moderator_ids
    Steps:
      1. Run: DATABASE_URL="..." cargo test --test stats_tests test_stats_command_unauthorized 2>&1
      2. Assert: FakeTelegramApi.sent_messages is empty, keyboards_sent is empty
    Expected Result: No message sent, silent rejection
    Evidence: .sisyphus/evidence/task-8-unauthorized.txt
  ```

  **Commit**: YES (group: T8+T9)
  - Message: `feat(bot): add /stats command and callback handlers`
  - Files: `src/bot/handlers/stats.rs`, `src/bot/mod.rs`, `tests/stats_tests.rs`
  - Pre-commit: `DATABASE_URL="..." cargo test --all`

---

- [ ] 9. Stats Callback Handlers with TDD — Navigation Flow

  **What to do**:
  - RED: Write failing tests for each callback type:
    - Community selection callback (`sc:42`) → shows period selection
    - Period selection callback (`sp:42:w`) → shows active view (page 1)
    - Navigation callback (`sn:42:w:c:2`) → shows active view page 2
    - View toggle callback (`sn:42:w:s:1`) → shows summary view page 1
    - Access control: unauthorized user's callback → answered but no action
  - GREEN: Implement in `src/bot/handlers/stats.rs`:
    - `process_stats_callback(api, pool, config, callback_input)` — main entry point for stats callbacks
    - Parse callback data using `StatsCallbackData::parse()` from T3
    - For each variant:
      - `SelectCommunity`: Load period selection keyboard, edit message with `edit_message_html_with_markup`
      - `SelectPeriod`: Load active applicants + format view, edit message
      - `Navigate`: Load requested view + page, edit message
    - Access control check on EVERY callback (not just the command)
  - Route stats callbacks in `src/bot/handlers/callbacks.rs` or `src/bot/mod.rs`:
    - In the callback query handler, check for `sc:`, `sp:`, `sn:` prefixes BEFORE the existing `a:`, `r:`, `b:` routing
    - Call `process_stats_callback` for stats prefixes
  - Handle edge cases:
    - Page beyond range → show last page
    - Zero applicants → show "No applicants found" message

  **Must NOT do**:
  - Do NOT modify existing callback routing for `a:`, `r:`, `b:`, `lang:` — only ADD new prefix handling
  - Do NOT store navigation state in the database
  - Do NOT break the moderator card approve/reject/ban flow

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Complex callback routing with multiple navigation paths, state management via callback data, many edge cases
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Tasks 7, 8)
  - **Blocks**: Task 10
  - **Blocked By**: Tasks 2 (TelegramApi trait), 5 (analytics service), 6 (formatter)

  **References**:

  **Pattern References**:
  - `src/bot/handlers/callbacks.rs:49-85` — Existing callback handling entry point — the EXACT routing pattern to extend with `sc:`, `sp:`, `sn:` prefixes
  - `src/bot/handlers/callbacks.rs:195-256` — How `edit_message_html` + `clear_message_reply_markup` are called in moderation callbacks — similar pattern but using new `edit_message_html_with_markup`
  - `src/bot/handlers/language_selection.rs:26-40` — Simpler callback processing pattern

  **API/Type References**:
  - `src/bot/handlers/stats.rs:StatsCallbackData` — Parse method (from T3)
  - `src/services/stats.rs:StatsService` — get_active_applicants, get_period_summary (from T5)
  - `src/services/stats_formatter.rs:StatsFormatter` — All format methods (from T6)
  - `src/bot/handlers/mod.rs:TelegramApi::edit_message_html_with_markup` — The new trait method (from T2)

  **WHY Each Reference Matters**:
  - `callbacks.rs:49-85`: New prefix routing MUST be added here following the EXACT same `if data.starts_with("sc:")` pattern
  - `edit_message_html_with_markup`: Each navigation step EDITS the existing message (not sends a new one) — this is the critical UX difference

  **Acceptance Criteria**:

  - [ ] Community selection callback shows period selection keyboard
  - [ ] Period selection callback shows active applicants view
  - [ ] Navigation callbacks move between pages correctly
  - [ ] View toggle switches between Active and Summary views
  - [ ] Access control checked on every callback
  - [ ] Edge case: page beyond range shows last page
  - [ ] Edge case: zero applicants shows empty state message
  - [ ] Existing callback routing (moderation, language) unaffected

  **QA Scenarios:**

  ```
  Scenario: Full navigation flow
    Tool: Bash
    Preconditions: Seeded data with community, applicants in various states, events recorded
    Steps:
      1. Simulate: community selection callback "sc:1"
      2. Assert: message edited with period selection keyboard (4 buttons)
      3. Simulate: period selection callback "sp:1:w"
      4. Assert: message edited with active view page 1 + navigation keyboard
      5. Simulate: view toggle callback "sn:1:w:s:1"
      6. Assert: message edited with summary view
    Expected Result: Each step edits the message with correct content and keyboard
    Evidence: .sisyphus/evidence/task-9-navigation-flow.txt

  Scenario: Zero applicants in period
    Tool: Bash
    Preconditions: Community exists but no applicants in "today" period
    Steps:
      1. Simulate: period selection callback "sp:1:t"
      2. Assert: message shows "No applicants found for today" with back button
    Expected Result: Graceful empty state with navigation back
    Evidence: .sisyphus/evidence/task-9-empty-state.txt

  Scenario: Existing moderation callbacks still work
    Tool: Bash
    Steps:
      1. Run: DATABASE_URL="..." cargo test --test moderation_tests 2>&1
      2. Assert: All existing moderation tests pass
    Expected Result: Zero regressions in callback routing
    Evidence: .sisyphus/evidence/task-9-no-regression.txt
  ```

  **Commit**: YES (group: T8+T9)
  - Message: `feat(bot): add /stats command and callback handlers`
  - Files: `src/bot/handlers/stats.rs`, `src/bot/handlers/callbacks.rs`, `src/bot/mod.rs`, `tests/stats_tests.rs`
  - Pre-commit: `DATABASE_URL="..." cargo test --all`

- [ ] 10. Full End-to-End Integration Test

  **What to do**:
  - Write a comprehensive integration test in `tests/stats_tests.rs` that verifies the ENTIRE flow:
    1. Seed data: community with 3 questions, 3 applicants in different states:
       - Applicant A: `questionnaire_in_progress`, on Q2, with events (question_presented + validation_failed + answer_accepted for Q1, question_presented for Q2)
       - Applicant B: `submitted` (completed all questions), with full event trail
       - Applicant C: `approved`, completed 1 week ago
    2. Test /stats command:
       - Moderator sends /stats → verify period selection keyboard (single community)
       - Verify callback data encoding in keyboard buttons
    3. Test period selection:
       - Simulate "This Week" callback → verify active view with Applicant A showing
       - Verify Applicant A shows: Q2/3, time on current question, retry count from events
    4. Test view toggle:
       - Simulate "Summary" view callback → verify summary with Applicants A + B + C
       - Verify per-question timing for Applicant B (computed from events)
    5. Test pagination (if applicable):
       - Seed 15 applicants, verify page 1 shows 10, page 2 shows 5
    6. Test access control:
       - Non-moderator callback → verify no action
  - Use `FakeTelegramApi` to capture all sent/edited messages and verify content
  - Verify DB state: question_events table has expected records

  **Must NOT do**:
  - Do not test individual components in isolation here (those are in T4-T9)
  - This test focuses on the INTEGRATION between all components working together

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Complex test setup with seeded data across many tables, verifying multi-step interaction flow
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4 (sequential)
  - **Blocks**: Tasks 11, 12
  - **Blocked By**: Tasks 7, 8, 9 (all implementation complete)

  **References**:

  **Test References**:
  - `tests/handler_tests.rs:350-550` — Existing integration test patterns with multi-step flows (join request → name → language → questionnaire → moderator card)
  - `tests/handler_tests.rs:555-629` — Seed data helpers
  - `tests/moderation_tests.rs` — Callback testing pattern with FakeTelegramApi

  **WHY Each Reference Matters**:
  - handler_tests.rs multi-step flow: Shows HOW to simulate a multi-step bot interaction in tests
  - Seed data helpers: MUST use the same seeding approach for consistency

  **Acceptance Criteria**:

  - [ ] Integration test covers full flow: /stats → period → active view → summary view → pagination
  - [ ] Test verifies event data is used for timing computation
  - [ ] Test verifies access control at multiple points
  - [ ] `cargo test --test stats_tests test_full_stats_flow` passes

  **QA Scenarios:**

  ```
  Scenario: Full integration test passes
    Tool: Bash
    Preconditions: All implementation tasks complete (T1-T9)
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --test stats_tests test_full_stats_flow 2>&1
      2. Assert: Test passes, all FakeTelegramApi assertions hold
    Expected Result: All steps in the flow produce correct messages and keyboards
    Failure Indicators: Any assertion failure, missing events, wrong formatting
    Evidence: .sisyphus/evidence/task-10-integration-test.txt
  ```

  **Commit**: YES (group: T10-T12)
  - Message: `test(stats): add end-to-end integration test`
  - Files: `tests/stats_tests.rs`
  - Pre-commit: `DATABASE_URL="..." cargo test --test stats_tests`

---

- [ ] 11. Regression Check — All Existing Tests Pass

  **What to do**:
  - Run the FULL test suite to verify zero regressions:
    ```bash
    DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all 2>&1
    ```
  - If any existing test fails, fix the issue (likely related to T2 trait extension or T7 instrumentation)
  - Run `cargo clippy` and fix any warnings in new code
  - Run `cargo build` to verify compile-time query checking

  **Must NOT do**:
  - Do not skip failing tests — fix them
  - Do not weaken existing test assertions to make them pass

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Just running existing test suite and fixing any issues
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4 (sequential after T10)
  - **Blocks**: Task 12
  - **Blocked By**: Task 10

  **Acceptance Criteria**:

  - [ ] `cargo test --all` — 0 failures
  - [ ] `cargo clippy` — 0 warnings in new code
  - [ ] `cargo build` — success

  **QA Scenarios:**

  ```
  Scenario: All tests pass
    Tool: Bash
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all 2>&1
      2. Assert: exit code 0, "test result: ok"
      3. Run: cargo clippy 2>&1
      4. Assert: No warnings in src/ files
    Expected Result: Full green test suite + clean clippy
    Evidence: .sisyphus/evidence/task-11-regression.txt
  ```

  **Commit**: NO (part of T10-T12 group)

---

- [ ] 12. `cargo sqlx prepare` + Build Verification + Commit `.sqlx/`

  **What to do**:
  - Regenerate sqlx offline metadata:
    ```bash
    DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo sqlx prepare
    ```
  - Verify offline mode works:
    ```bash
    cargo sqlx prepare --check
    ```
  - Verify Docker build still works:
    ```bash
    cargo build --release
    ```
  - Stage and commit the updated `.sqlx/*.json` files

  **Must NOT do**:
  - Do not manually edit `.sqlx/*.json` files — always regenerate with `cargo sqlx prepare`

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Just running build commands and committing generated files
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 4 (sequential after T11)
  - **Blocks**: Final verification wave
  - **Blocked By**: Task 11

  **Acceptance Criteria**:

  - [ ] `cargo sqlx prepare` succeeds
  - [ ] `cargo sqlx prepare --check` succeeds
  - [ ] `cargo build --release` succeeds
  - [ ] `.sqlx/` directory has new JSON files for new queries

  **QA Scenarios:**

  ```
  Scenario: Offline metadata is up to date
    Tool: Bash
    Steps:
      1. Run: DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo sqlx prepare 2>&1
      2. Run: cargo sqlx prepare --check 2>&1
      3. Assert: Both exit code 0
      4. Run: cargo build --release 2>&1
      5. Assert: exit code 0
    Expected Result: All metadata generated, build succeeds
    Evidence: .sisyphus/evidence/task-12-sqlx-prepare.txt
  ```

  **Commit**: YES (group: T10-T12)
  - Message: `test(stats): add integration tests and update sqlx metadata`
  - Files: `tests/stats_tests.rs`, `.sqlx/*.json`
  - Pre-commit: `cargo test --all && cargo sqlx prepare --check`

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Rejection → fix → re-run.

- [ ] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, run command). For each "Must NOT Have": search codebase for forbidden patterns — reject with file:line if found. Check evidence files exist in `.sisyphus/evidence/`. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [ ] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo build` + `cargo test --all` + `cargo clippy`. Review all changed files for: `unwrap()` in production code (not tests), `as any`/unsafe, empty error handling, console-style logging in production paths, unused imports. Check AI slop: excessive comments, over-abstraction, generic names (data/result/item/temp).
  Output: `Build [PASS/FAIL] | Tests [N pass/N fail] | Clippy [PASS/FAIL] | Files [N clean/N issues] | VERDICT`

- [ ] F3. **Real Manual QA** — `unspecified-high`
  Start from clean state. Verify: (1) migration applies cleanly, (2) bot starts without errors, (3) questionnaire flow still works and events are recorded, (4) /stats from unauthorized user gets no response, (5) /stats from authorized moderator shows community selection (or period if single), (6) navigation through all callback states works. Save evidence to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

- [ ] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff. Verify 1:1 — everything in spec was built, nothing beyond spec was built. Check "Must NOT do" compliance. Detect cross-task contamination. Flag unaccounted changes.
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

| Group | Message | Files | Pre-commit Check |
|-------|---------|-------|-----------------|
| T1 | `feat(db): add question_events migration and domain model` | `migrations/011_*.sql`, `src/domain/question_event.rs`, `src/domain/mod.rs` | `cargo build` |
| T2 | `refactor(bot): extend TelegramApi trait with edit_message_html_with_markup` | `src/bot/handlers/mod.rs`, `tests/*.rs` (FakeTelegramApi updates) | `cargo test --all` |
| T3 | `feat(bot): add stats callback data types and parser` | `src/bot/handlers/stats.rs` (partial — types only) | `cargo build` |
| T4 | `feat(db): add QuestionEventRepo with create and query methods` | `src/db/question_event_repo.rs`, `src/db/mod.rs`, `tests/stats_tests.rs` (partial) | `cargo test --test stats_tests` |
| T5 | `feat(services): add stats analytics service with period queries` | `src/services/stats.rs`, `src/services/mod.rs`, `tests/stats_tests.rs` (partial) | `cargo test --test stats_tests` |
| T6 | `feat(services): add stats message formatter and keyboard builder` | `src/services/stats_formatter.rs`, `src/services/mod.rs`, `tests/stats_tests.rs` (partial) | `cargo test --test stats_tests` |
| T7 | `feat(bot): instrument question events in questionnaire flow` | `src/bot/handlers/questionnaire.rs`, `src/bot/handlers/language_selection.rs` | `cargo test --all` |
| T8+T9 | `feat(bot): add /stats command and callback handlers` | `src/bot/handlers/stats.rs`, `src/bot/mod.rs`, `tests/stats_tests.rs` (partial) | `cargo test --all` |
| T10-T12 | `test(stats): add integration tests and update sqlx metadata` | `tests/stats_tests.rs`, `.sqlx/*.json` | `cargo test --all && cargo sqlx prepare --check` |

---

## Success Criteria

### Verification Commands
```bash
# All tests pass (existing + new)
DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all
# Expected: test result: ok. 0 failures

# Offline metadata is up to date
cargo sqlx prepare --check
# Expected: success

# Build passes with compile-time query checking
cargo build
# Expected: Finished

# New stats tests specifically pass
DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --test stats_tests
# Expected: test result: ok. 0 failures
```

### Final Checklist
- [ ] All "Must Have" items implemented and tested
- [ ] All "Must NOT Have" items verified absent
- [ ] All existing tests pass (zero regressions)
- [ ] New `question_events` table created with proper indexes
- [ ] Events recorded during questionnaire flow (non-blocking)
- [ ] `/stats` command works for moderators, ignored for others
- [ ] Full navigation: community → period → view → pagination
- [ ] `.sqlx/` metadata committed and `prepare --check` passes
