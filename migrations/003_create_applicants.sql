-- Create applicants table
CREATE TABLE IF NOT EXISTS applicants (
    id BIGSERIAL PRIMARY KEY,
    telegram_user_id BIGINT NOT NULL,
    first_name TEXT NOT NULL,
    last_name TEXT,
    username TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique index on telegram_user_id
CREATE UNIQUE INDEX IF NOT EXISTS uq_applicants_telegram_user_id
    ON applicants (telegram_user_id);
