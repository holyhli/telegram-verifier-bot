# Language Selection Feature (English/Ukrainian)

## TL;DR

> **Quick Summary**: Add language selection feature to the verifier-bot. When users request to join, they first select their preferred language (English or Ukrainian) via inline buttons, then receive all questions and messages in that language.
>
> **Deliverables**:
> - Database schema changes to support language storage and multilingual questions
> - Updated config structure for bilingual questions
> - Language selection flow with inline buttons
> - All user-facing messages in both languages
> - Updated tests for bilingual functionality
>
> **Estimated Effort**: Medium (8 tasks)
> **Parallel Execution**: YES — Some tasks can be parallelized
> **Critical Path**: Database → Config → Models → Language Selection UI → Message Templates → Integration

---

## Context

### User Request
Add a feature where people who want to join can select their language (English or Ukrainian) using buttons, and then receive questions in the selected language.

### Current State
- Questions are defined in config.toml with single language text
- All bot messages are in English only
- No language preference is stored or tracked

### Target State
- Users see language selection buttons (English/Ukrainian) when they first request to join
- After selecting a language, users receive all questions and messages in that language
- Language preference is stored in the database
- Questions are configured with both English and Ukrainian text

---

## Tasks

### Task 1: Database Migration for Language Support
**Parallelizable**: No (foundation for other tasks)
**Dependencies**: None

Create database migrations to support language selection and multilingual questions:

**Subtasks**:
- [ ] Create migration `010_add_language_support.sql` with:
  - Add `language` column to `applicant_sessions` table (VARCHAR(5) NOT NULL DEFAULT 'en', CHECK language IN ('en', 'uk'))
  - Add `question_text_uk` column to `community_questions` table (TEXT NOT NULL)
  - Add index on `applicant_sessions.language` for analytics
- [ ] Update `.sqlx/` metadata with `cargo sqlx prepare` (requires running migration first)
- [ ] Verify migration applies cleanly to fresh database and existing databases

**Acceptance Criteria**:
- Migration file created and applies without errors
- Existing data preserved (English as default language)
- Constraints ensure only valid language codes ('en', 'uk')
- sqlx offline metadata updated

---

### Task 2: Update Config Structure for Bilingual Questions
**Parallelizable**: Partially (can start after Task 1 planning, before implementation)
**Dependencies**: Task 1 (database schema)

Update configuration loading to support bilingual questions:

**Subtasks**:
- [ ] Update `Question` struct in `src/config.rs`:
  - Rename `text` field to `text_en`
  - Add `text_uk` field (String)
- [ ] Update validation to ensure both `text_en` and `text_uk` are non-empty
- [ ] Update `config.example.toml` with bilingual question examples
- [ ] Add config tests for bilingual validation (missing Ukrainian text, empty Ukrainian text)
- [ ] Update `sync_config_to_db()` to insert/update both English and Ukrainian text

**Acceptance Criteria**:
- Config loads with both English and Ukrainian question text
- Validation fails if either language is missing
- Example config demonstrates bilingual question format
- Tests cover all validation cases
- Database sync writes both language variants

---

### Task 3: Domain Models for Language Support
**Parallelizable**: Yes (can run in parallel with Task 2)
**Dependencies**: Task 1 (database schema)

Add language domain model and update affected models:

**Subtasks**:
- [ ] Create `src/domain/language.rs`:
  - Define `Language` enum with variants `English`, `Ukrainian`
  - Implement sqlx::Type with rename_all = "lowercase" → 'en', 'uk'
  - Implement Display, Serialize, Deserialize
  - Add helper methods: `code()`, `from_code()`, `name()`
- [ ] Update `CommunityQuestion` in `src/domain/community.rs`:
  - Add `question_text_uk` field
  - Add method `text_for_language(&self, lang: Language) -> &str`
- [ ] Update `ApplicantSession` in `src/domain/session.rs`:
  - Add `language` field (Language type)
- [ ] Update `src/domain/mod.rs` to export Language
- [ ] Add unit tests for Language enum conversions and CommunityQuestion language selection

**Acceptance Criteria**:
- Language enum properly maps to/from database 'en'/'uk' strings
- CommunityQuestion returns correct text for each language
- ApplicantSession includes language field
- All tests pass

---

### Task 4: Message Templates in Both Languages
**Parallelizable**: Yes (can run in parallel with Task 3)
**Dependencies**: Task 3 (Language enum exists)

Create bilingual message templates for all user-facing messages:

**Subtasks**:
- [ ] Create `src/messages.rs` module:
  - Define struct `Messages` with static methods for each message type
  - Implement `welcome_message(first_name: &str, community_title: &str, lang: Language) -> String`
  - Implement `language_selection_message(first_name: &str, community_title: &str) -> String`
  - Implement `completion_message(lang: Language) -> String`
  - Implement validation error messages: `required_field_error()`, `low_effort_error()`, `min_length_error()`
- [ ] Translate all message strings to Ukrainian:
  - Welcome message
  - Language selection prompt
  - Completion message
  - All validation errors
- [ ] Add module to `src/lib.rs`
- [ ] Add tests for message generation in both languages

**Ukrainian Translations Needed**:
- "Hi {name}! I saw your request to join {community}." → "Привіт {name}! Я бачу твій запит приєднатися до {community}."
- "Please select your preferred language:" → "Будь ласка, обери свою мову:"
- "Thanks — your application has been submitted..." → "Дякую — твою заявку відправлено модераторам..."
- "This field is required." → "Це поле обов'язкове."
- "Please provide a more detailed answer." → "Будь ласка, дай більш детальну відповідь."
- "Answer must be at least 2 characters." → "Відповідь має містити хоча б 2 символи."

**Acceptance Criteria**:
- All user-facing messages available in both languages
- Ukrainian translations are natural and contextually appropriate
- Tests verify correct messages returned for each language
- Messages module is well-documented

---

### Task 5: Language Selection Flow (Join Request Handler)
**Parallelizable**: No (requires Tasks 3 and 4)
**Dependencies**: Tasks 3, 4

Update join request handler to show language selection first:

**Subtasks**:
- [ ] Update `src/bot/handlers/join_request.rs`:
  - Remove immediate question sending from `process_join_request()`
  - Add language selection message with inline keyboard (English/Ukrainian buttons)
  - Create callback data format: `lang:en` and `lang:uk`
  - Keep session creation but with `awaiting_language_selection` state (or keep at position 0)
  - Keep status as `PendingContact` until language is selected
- [ ] Update `TelegramApi` trait:
  - Add method `send_message_with_inline_keyboard(chat_id, text, keyboard) -> Result<()>`
- [ ] Update `TeloxideApi` implementation for inline keyboard sending
- [ ] Update `FakeTelegramApi` for testing
- [ ] Add tests for language selection message sending

**Acceptance Criteria**:
- Join request handler sends language selection message with buttons
- No questions sent until language is selected
- Session created but awaiting language selection
- Tests verify message and buttons are sent correctly

---

### Task 6: Language Selection Callback Handler
**Parallelizable**: No (requires Task 5)
**Dependencies**: Task 5

Create callback handler for language selection:

**Subtasks**:
- [ ] Create `src/bot/handlers/language_selection.rs`:
  - Implement `process_language_selection_callback()` function
  - Parse callback data (`lang:en` or `lang:uk`)
  - Update session with selected language
  - Load first question in selected language
  - Send welcome message + first question in selected language
  - Transition join request to `QuestionnaireInProgress`
  - Answer callback query with confirmation
- [ ] Update `src/bot/handlers/callbacks.rs`:
  - Route `lang:*` callbacks to language selection handler
  - Keep existing moderation callback routing (starts with 'a:', 'r:', 'b:')
- [ ] Update `SessionRepo` in `src/db/session.rs`:
  - Add method `update_language(pool, session_id, language) -> Result<ApplicantSession>`
- [ ] Add comprehensive tests covering:
  - Valid language selection (en and uk)
  - Invalid callback data
  - Session not found
  - First question sent in correct language

**Acceptance Criteria**:
- Language selection callbacks properly routed
- Session updated with selected language
- First question sent in correct language
- Join request status transitions to QuestionnaireInProgress
- All tests pass

---

### Task 7: Update Questionnaire Flow for Language
**Parallelizable**: No (requires Task 6)
**Dependencies**: Task 6

Update questionnaire logic to use selected language:

**Subtasks**:
- [ ] Update `src/services/questionnaire.rs`:
  - Modify `find_active_context_by_telegram_user_id()` to include session language
  - Update `QuestionnaireContext` struct to include language field
  - Use language when loading questions
  - Send messages in selected language (use Messages module)
- [ ] Update `src/bot/handlers/questionnaire.rs`:
  - Pass language to Messages functions
  - Use language-specific messages for validation errors
- [ ] Update completion message to use selected language
- [ ] Update all questionnaire tests to test both languages

**Acceptance Criteria**:
- Questions displayed in user's selected language
- Validation errors in user's selected language
- Completion message in user's selected language
- Tests verify both languages work correctly throughout flow

---

### Task 8: Integration Testing and Documentation
**Parallelizable**: Partially (docs can be written while tests run)
**Dependencies**: All previous tasks

Add comprehensive integration tests and update documentation:

**Subtasks**:
- [ ] Create `tests/language_selection_tests.rs`:
  - Test full flow: join request → language selection → questionnaire → completion (both languages)
  - Test language switching scenarios (if applicable)
  - Test default language fallback
- [ ] Update existing tests that hardcode English messages
- [ ] Update README.md:
  - Add section on language support
  - Update config.example.toml documentation
  - Add screenshots/examples of language selection
- [ ] Update `config.example.toml` with bilingual examples for both communities
- [ ] Verify all tests pass: `cargo test --all`
- [ ] Verify build with offline sqlx: `SQLX_OFFLINE=true cargo build --release`

**Acceptance Criteria**:
- Integration tests cover full bilingual flow
- All existing tests updated and passing
- README documents language feature
- Config examples show bilingual setup
- Full test suite passes
- Offline build succeeds

---

## Verification Checklist

After all tasks complete:

- [ ] Database migrations apply cleanly (fresh and existing DBs)
- [ ] Config loads with bilingual questions
- [ ] Join request shows language selection buttons
- [ ] Language selection works for both languages
- [ ] Questions appear in selected language
- [ ] Validation errors in selected language
- [ ] Completion message in selected language
- [ ] Moderator card shows selected language (future enhancement)
- [ ] All tests pass (`cargo test --all`)
- [ ] Offline build succeeds (`SQLX_OFFLINE=true cargo build`)
- [ ] Docker build succeeds
- [ ] Documentation updated

---

## Notes

### Design Decisions

1. **Language Storage**: Store in `applicant_sessions` rather than `join_requests` because language is session-specific
2. **Default Language**: English ('en') as default for backward compatibility
3. **Question Storage**: Denormalized (two columns) rather than separate translations table for simplicity and query performance
4. **Language Selection State**: Use position=0 or create new session state to track "awaiting language selection"
5. **Callback Format**: Simple `lang:en` and `lang:uk` format, separate from moderation callbacks

### Future Enhancements (Not in Scope)

- Language selection after questionnaire has started (language switching)
- More languages (Spanish, French, etc.)
- Admin interface to manage translations
- Moderator card language preferences
- Language analytics and reporting

### Risk Mitigation

- Backward compatibility: Default to English for existing sessions
- Validation: Ensure all questions have both languages in config
- Testing: Comprehensive tests for both languages throughout entire flow
- Migration safety: Non-destructive migration with default values
