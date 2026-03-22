-- Create communities table
CREATE TABLE IF NOT EXISTS communities (
    id BIGSERIAL PRIMARY KEY,
    telegram_chat_id BIGINT NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique indexes
CREATE UNIQUE INDEX IF NOT EXISTS uq_communities_telegram_chat_id
    ON communities (telegram_chat_id);

CREATE UNIQUE INDEX IF NOT EXISTS uq_communities_slug
    ON communities (slug);
