# Issues — stats-command

## Known Gotchas
- FakeTelegramApi is duplicated in EVERY test file — T2 must update ALL of them
- TelegramApi trait extension (T2) blocks T8 and T9 — must complete first
- `.sqlx/` offline metadata must be regenerated after T4/T5 add new queries (T12)
- `cargo sqlx prepare` requires live DB connection
- Wave 1 tasks (T1, T2, T3) can all run in parallel
- Wave 2 tasks (T4, T5, T6) can run in parallel after Wave 1
- Wave 3 tasks (T7, T8, T9) can run in parallel after Wave 2
- Wave 4 tasks (T10, T11, T12) are sequential

## Pre-existing Issue: language_selection_tests.rs
- 52 compile errors in `tests/language_selection_tests.rs` — pre-existing from commit b09fb17
- The `process_private_message` function signature changed but tests weren't updated
- This is NOT caused by our Wave 1 work
- `cargo test --all` will fail due to this — use `--test stats_tests` or specific test files
- This needs to be fixed in T11 (regression check) or earlier
