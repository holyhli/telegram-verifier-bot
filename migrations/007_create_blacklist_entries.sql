-- Create blacklist_entries table
CREATE TABLE IF NOT EXISTS blacklist_entries (
    id BIGSERIAL PRIMARY KEY,
    telegram_user_id BIGINT NOT NULL,
    scope_type TEXT NOT NULL,
    community_id BIGINT,
    reason TEXT,
    created_by_moderator_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_blacklist_entries_community_id
        FOREIGN KEY (community_id) REFERENCES communities(id),

    -- Scope type enum enforced via CHECK constraint
    CONSTRAINT chk_blacklist_entries_scope_type CHECK (
        scope_type IN ('global', 'community')
    ),

    -- If scope_type is 'community', community_id must be set
    CONSTRAINT chk_blacklist_entries_community_scope CHECK (
        (scope_type = 'global' AND community_id IS NULL) OR
        (scope_type = 'community' AND community_id IS NOT NULL)
    )
);

-- Index for looking up blacklist by user + scope
CREATE INDEX IF NOT EXISTS idx_blacklist_entries_user_scope
    ON blacklist_entries (telegram_user_id, scope_type);

-- Index for looking up blacklist by community
CREATE INDEX IF NOT EXISTS idx_blacklist_entries_community_id
    ON blacklist_entries (community_id)
    WHERE community_id IS NOT NULL;
