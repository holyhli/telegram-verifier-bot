-- Create applicant_sessions table
CREATE TABLE IF NOT EXISTS applicant_sessions (
    id BIGSERIAL PRIMARY KEY,
    join_request_id BIGINT NOT NULL,
    current_question_position INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'awaiting_answer',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_applicant_sessions_join_request_id
        FOREIGN KEY (join_request_id) REFERENCES join_requests(id) ON DELETE CASCADE,

    -- Session state enum enforced via CHECK constraint
    CONSTRAINT chk_applicant_sessions_state CHECK (
        state IN ('awaiting_answer', 'completed', 'expired', 'cancelled')
    )
);

-- Index on join_request_id for session lookup
CREATE INDEX IF NOT EXISTS idx_applicant_sessions_join_request_id
    ON applicant_sessions (join_request_id);
