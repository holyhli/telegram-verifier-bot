# Decisions: language-selection

This file tracks architectural and design decisions made during implementation.

---

## Design Decisions

1. **Language Storage**: Store in `applicant_sessions` rather than `join_requests` because language is session-specific
2. **Default Language**: English ('en') as default for backward compatibility
3. **Question Storage**: Denormalized (two columns) rather than separate translations table for simplicity and query performance
4. **Language Selection State**: Use position=0 or await language before creating session
5. **Callback Format**: Simple `lang:en` and `lang:uk` format, separate from moderation callbacks

---
