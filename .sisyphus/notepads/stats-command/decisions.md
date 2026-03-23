# Decisions — stats-command

## Architecture Decisions
- Stats navigation state encoded entirely in callback_data (no DB state)
- Compact callback format: `sc:{id}`, `sp:{id}:{period}`, `sn:{id}:{period}:{view}:{page}`
- Period chars: t=Today, w=ThisWeek, m=ThisMonth, a=AllTime
- View chars: c=Active(Current), s=Summary
- English-only output (moderator tool, not user-facing)
- Pagination: 10 users per page, snapshot-based (stale page → show last page)
- Single community: auto-skip community selection, go straight to period selection
- Event failures: fire-and-forget, never propagate to user
