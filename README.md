# Telegram Join Request Moderation Bot

A production-ready Telegram bot built in Rust that automates community join request verification through customizable questionnaires, moderator review workflows, and comprehensive audit trails.

## Features

- **Multi-Community Support**: Single bot handles multiple Telegram communities with custom questionnaires per community
- **Bilingual Support**: Full English and Ukrainian language support with user-selectable language preference
- **Automated Questionnaire Flow**: Private one-on-one questioning with answer validation and anti-spam protection
- **Moderator Review**: Inline button interface for approve/reject/ban decisions with double-processing protection
- **Blacklist Management**: Global and community-scoped blacklists with automatic decline
- **Application Expiry**: Configurable timeouts with reminder messages before expiry
- **Audit Trail**: Complete moderation action history stored in PostgreSQL
- **Dual Deployment Modes**: Long polling (default) or webhook mode for production
- **Docker Ready**: Multi-stage Dockerfile with cargo-chef for fast rebuilds

## Language Support

The bot provides full bilingual support for English and Ukrainian:

- **User Language Selection**: When users request to join a community, they're presented with a language selection interface (🇬🇧 English / 🇺🇦 Українська)
- **Localized Questions**: All questionnaire questions are configured with both English (`text_en`) and Ukrainian (`text_uk`) versions
- **Localized Messages**: Welcome messages, validation errors, and completion messages appear in the user's selected language
- **Persistent Language Preference**: Language selection is stored per session and used throughout the entire questionnaire flow

### Configuring Bilingual Questions

In `config.toml`, each question requires both language versions:

```toml
[[communities.questions]]
key = "name"
text_en = "What is your name?"
text_uk = "Як вас звати?"
required = true
position = 1
```

Both `text_en` and `text_uk` fields are required and must be non-empty. The bot will validate this on startup.

## Prerequisites

- **Rust**: 1.70+ (for development)
- **Docker & Docker Compose**: For deployment
- **PostgreSQL**: 16+ (provided via Docker Compose)
- **Telegram Bot Token**: From [@BotFather](https://t.me/BotFather)

## Telegram Bot Setup

### 1. Create Bot via BotFather

1. Message [@BotFather](https://t.me/BotFather) on Telegram
2. Send `/newbot` and follow prompts
3. Save the bot token (format: `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`)

### 2. Configure Bot Permissions

Your bot needs these permissions in target communities:

- **Can invite users** (required for processing join requests)
- **Can delete messages** (optional, for moderation)

### 3. Enable Join Request Approval

In each community you want to moderate:

1. Go to **Community Settings** → **Manage Community**
2. Enable **Approve New Members**
3. Add your bot as an administrator with "Can invite users" permission

### 4. Get Chat IDs

**For Communities:**
- Add [@getmyid_bot](https://t.me/getmyid_bot) to your community
- Send any message in the community
- The bot will reply with the chat ID (format: `-1001234567890`)

**For Moderator Chat:**
- Create a private supergroup for moderators
- Add [@getmyid_bot](https://t.me/getmyid_bot) to the group
- Get the chat ID (format: `-1001234567890`)
- Add your bot to this group as admin

**For Moderator User IDs:**
- Message [@getmyid_bot](https://t.me/getmyid_bot) privately
- It will reply with your user ID (format: `123456789`)

## Quick Start with Docker Compose

### 1. Clone and Configure

```bash
# Clone repository
git clone <repository-url>
cd verifier-bot

# Copy example files
cp .env.example .env
cp config.example.toml config.toml

# Edit .env with your values
nano .env

# Edit config.toml with your communities and questions
nano config.toml
```

### 2. Configure Environment Variables

Edit `.env` with your actual values:

```bash
# Required
BOT_TOKEN=your_bot_token_from_botfather
DATABASE_URL=postgres://verifier:verifier_dev@postgres:5432/verifier_bot
DEFAULT_MODERATOR_CHAT_ID=-1001234567890
ALLOWED_MODERATOR_IDS=123456789,987654321

# Optional (defaults shown)
RUST_LOG=info,verifier_bot=debug
APPLICATION_TIMEOUT_MINUTES=60
REMINDER_BEFORE_EXPIRY_MINUTES=15
USE_WEBHOOKS=false
SERVER_PORT=8080
CONFIG_PATH=config.toml
```

### 3. Configure Communities

Edit `config.toml` to define your communities and questions:

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
text_en = "What is your name?"
text_uk = "Як вас звати?"
required = true
position = 1

[[communities.questions]]
key = "occupation"
text_en = "What do you do / where do you work?"
text_uk = "Чим ви займаєтесь / де ви працюєте?"
required = true
position = 2

[[communities.questions]]
key = "referral"
text_en = "How did you hear about us?"
text_uk = "Як ви про нас дізналися?"
required = false
position = 3
```

### 4. Start the Bot

```bash
# Start services
docker compose up -d

# Check logs
docker compose logs -f bot

# Verify health
curl http://localhost:8080/health
```

### 5. Test the Flow

1. Request to join one of your configured communities
2. Bot messages you privately with language selection (🇬🇧 English / 🇺🇦 Українська)
3. Select your preferred language
4. Answer all questions in your selected language
5. Moderator chat receives a card with your answers
6. Moderator clicks approve/reject/ban

## Development Setup

### 1. Start Test Database

```bash
docker compose -f docker-compose.dev.yml up -d
```

### 2. Configure Environment

```bash
cp .env.example .env
# Edit .env with test database URL:
# DATABASE_URL=postgres://verifier:verifier_dev@localhost:5432/verifier_bot
```

### 3. Run Migrations

```bash
cargo install sqlx-cli --no-default-features --features postgres
cargo sqlx migrate run
```

### 4. Run the Bot

```bash
cargo run
```

### 5. Run Tests

```bash
# All tests
DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --all

# Specific test suite
DATABASE_URL="postgres://verifier:verifier_dev@localhost:5432/verifier_bot" cargo test --test handler_tests
```

## Configuration Reference

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `BOT_TOKEN` | Yes | - | Telegram bot token from BotFather |
| `DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `DEFAULT_MODERATOR_CHAT_ID` | Yes | - | Chat ID where moderator cards are sent |
| `ALLOWED_MODERATOR_IDS` | Yes | - | Comma-separated Telegram user IDs allowed to moderate |
| `CONFIG_PATH` | No | `config.toml` | Path to TOML configuration file |
| `RUST_LOG` | No | `verifier_bot=info` | Logging level (use `debug` for development) |
| `APPLICATION_TIMEOUT_MINUTES` | No | `60` | How long before applications expire |
| `REMINDER_BEFORE_EXPIRY_MINUTES` | No | `15` | When to send reminder before expiry |
| `USE_WEBHOOKS` | No | `false` | Enable webhook mode instead of long polling |
| `PUBLIC_WEBHOOK_URL` | No | - | Required if `USE_WEBHOOKS=true` |
| `SERVER_PORT` | No | `8080` | Port for webhook server and health endpoint |

### TOML Configuration

The `config.toml` file defines communities and their questionnaires:

```toml
[bot]
# Override timeout settings (optional, can use env vars instead)
application_timeout_minutes = 60
reminder_before_expiry_minutes = 15

# Define multiple communities
[[communities]]
telegram_chat_id = -1001234567890  # Community chat ID
title = "Community Name"            # Display name
slug = "community-slug"             # Unique identifier

# Questions for this community (bilingual)
[[communities.questions]]
key = "unique_key"                  # Unique within community
text_en = "Question text?"          # English version
text_uk = "Текст питання?"          # Ukrainian version
required = true                     # Whether answer is required
position = 1                        # Order (must be 1, 2, 3, ... with no gaps)
```

**Important:**
- Question positions must be contiguous (1, 2, 3, ...) with no gaps
- Question keys must be unique within each community
- Community slugs must be unique across all communities
- Both `text_en` and `text_uk` are required for all questions and must be non-empty

## Architecture

### Tech Stack

- **Language**: Rust 1.70+
- **Bot Framework**: [teloxide](https://github.com/teloxide/teloxide) 0.17
- **Database**: PostgreSQL 16 with [sqlx](https://github.com/launchbadge/sqlx) 0.8
- **Async Runtime**: [tokio](https://tokio.rs/) 1.50
- **Web Server**: [axum](https://github.com/tokio-rs/axum) 0.8 (for webhooks)
- **Logging**: [tracing](https://github.com/tokio-rs/tracing) with structured fields

### Project Structure

```
verifier-bot/
├── src/
│   ├── main.rs              # Entry point
│   ├── config.rs            # Configuration loading
│   ├── error.rs             # Error types
│   ├── logging.rs           # Logging setup
│   ├── bot/
│   │   ├── mod.rs           # Bot dispatcher
│   │   ├── handlers/        # Update handlers
│   │   ├── webhook.rs       # Webhook server
│   │   └── shutdown.rs      # Graceful shutdown
│   ├── domain/              # Domain models
│   ├── db/                  # Repository layer
│   └── services/            # Business logic
├── migrations/              # Database migrations
├── tests/                   # Integration tests
├── Dockerfile               # Multi-stage production build
├── docker-compose.yml       # Production deployment
└── docker-compose.dev.yml   # Development database
```

### Database Schema

8 tables with full audit trail:

- `communities` - Community definitions
- `community_questions` - Questions per community
- `applicants` - User profiles
- `join_requests` - Join request lifecycle
- `join_request_answers` - Questionnaire responses
- `applicant_sessions` - FSM state tracking
- `moderation_actions` - Audit trail
- `blacklist_entries` - Banned users

## Troubleshooting

### Bot doesn't receive join requests

**Check:**
1. Bot is admin in the community with "Can invite users" permission
2. "Approve New Members" is enabled in community settings
3. Bot token is correct in `.env`
4. Logs show `verifier-bot starting` with correct community count

### Applicant doesn't receive questions

**Check:**
1. User hasn't blocked the bot
2. Logs show `join request processed` with correct IDs
3. Community is configured in `config.toml` with matching `telegram_chat_id`
4. Database has the community record (check with `docker compose exec postgres psql -U verifier -d verifier_bot -c "SELECT * FROM communities;"`)

### Moderator card not appearing

**Check:**
1. `DEFAULT_MODERATOR_CHAT_ID` is correct in `.env`
2. Bot is admin in the moderator chat
3. Applicant completed all required questions
4. Logs show `moderator card sent` or error message

### Moderator buttons don't work

**Check:**
1. Your Telegram user ID is in `ALLOWED_MODERATOR_IDS`
2. Join request status is `submitted` (not already processed)
3. Logs show callback query received

### Database connection errors

**Check:**
1. PostgreSQL container is running: `docker compose ps`
2. `DATABASE_URL` matches the postgres service configuration
3. Migrations have been applied: `docker compose logs bot | grep migration`

### Application timeouts not working

**Check:**
1. `APPLICATION_TIMEOUT_MINUTES` is set correctly
2. Expiry background task is running (logs show `expiry background loop started`)
3. System time is correct (timeouts use UTC)

## Production Deployment

### Webhook Mode (Recommended for Production)

1. Set up reverse proxy (nginx/caddy) with TLS
2. Configure environment:
   ```bash
   USE_WEBHOOKS=true
   PUBLIC_WEBHOOK_URL=https://your-domain.com/webhook
   ```
3. Ensure `SERVER_PORT` is accessible to Telegram servers
4. Start with `docker compose up -d`

### Long Polling Mode (Simpler Setup)

1. Keep `USE_WEBHOOKS=false` (default)
2. No reverse proxy needed
3. Start with `docker compose up -d`

### Health Checks

The bot exposes a health endpoint at `/health`:

```bash
curl http://localhost:8080/health
# Response: {"status":"ok","mode":"polling"}
```

Use this for:
- Docker health checks (already configured in `docker-compose.yml`)
- Load balancer health probes
- Monitoring systems

## License

[Add your license here]

## Contributing

[Add contribution guidelines here]
