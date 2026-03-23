-- Create question_events table for audit trail of question interactions
CREATE TABLE IF NOT EXISTS question_events (
    id BIGSERIAL PRIMARY KEY,
    join_request_id BIGINT NOT NULL REFERENCES join_requests(id),
    community_question_id BIGINT NOT NULL REFERENCES community_questions(id),
    applicant_id BIGINT NOT NULL REFERENCES applicants(id),
    event_type TEXT NOT NULL CHECK (event_type IN ('question_presented', 'validation_failed', 'answer_accepted')),
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index on join_request_id for session lookup
CREATE INDEX IF NOT EXISTS idx_question_events_join_request
    ON question_events (join_request_id);

-- Index on event_type and created_at for filtering by type and time
CREATE INDEX IF NOT EXISTS idx_question_events_type_created
    ON question_events (event_type, created_at);

-- Index on community_question_id and created_at for question-specific analytics
CREATE INDEX IF NOT EXISTS idx_question_events_community_question
    ON question_events (community_question_id, created_at);
