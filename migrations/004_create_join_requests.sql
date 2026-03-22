-- Create join_requests table with status CHECK constraint
CREATE TABLE IF NOT EXISTS join_requests (
    id BIGSERIAL PRIMARY KEY,
    community_id BIGINT NOT NULL,
    applicant_id BIGINT NOT NULL,
    telegram_user_chat_id BIGINT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending_contact',
    telegram_join_request_date TIMESTAMPTZ NOT NULL,
    submitted_to_moderators_at TIMESTAMPTZ,
    approved_at TIMESTAMPTZ,
    rejected_at TIMESTAMPTZ,
    moderator_message_chat_id BIGINT,
    moderator_message_id BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_join_requests_community_id
        FOREIGN KEY (community_id) REFERENCES communities(id),
    CONSTRAINT fk_join_requests_applicant_id
        FOREIGN KEY (applicant_id) REFERENCES applicants(id),

    -- Status enum enforced via CHECK constraint
    CONSTRAINT chk_join_requests_status CHECK (
        status IN (
            'pending_contact',
            'questionnaire_in_progress',
            'submitted',
            'approved',
            'rejected',
            'banned',
            'expired',
            'cancelled'
        )
    )
);

-- Index on (community_id, status) for filtering active requests
CREATE INDEX IF NOT EXISTS idx_join_requests_community_status
    ON join_requests (community_id, status);

-- Index on applicant_id for lookups
CREATE INDEX IF NOT EXISTS idx_join_requests_applicant_id
    ON join_requests (applicant_id);

-- Unique partial index: only one active request per applicant+community
CREATE UNIQUE INDEX IF NOT EXISTS uq_join_requests_active_per_applicant_community
    ON join_requests (applicant_id, community_id)
    WHERE status NOT IN ('approved', 'rejected', 'banned', 'expired', 'cancelled');
