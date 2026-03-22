-- Create moderation_actions table
CREATE TABLE IF NOT EXISTS moderation_actions (
    id BIGSERIAL PRIMARY KEY,
    join_request_id BIGINT NOT NULL,
    moderator_telegram_user_id BIGINT NOT NULL,
    action_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_moderation_actions_join_request_id
        FOREIGN KEY (join_request_id) REFERENCES join_requests(id),

    -- Action type enum enforced via CHECK constraint
    CONSTRAINT chk_moderation_actions_action_type CHECK (
        action_type IN ('approved', 'rejected', 'banned')
    )
);

-- Index on join_request_id for looking up actions per request
CREATE INDEX IF NOT EXISTS idx_moderation_actions_join_request_id
    ON moderation_actions (join_request_id);
