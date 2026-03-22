-- Create join_request_answers table
CREATE TABLE IF NOT EXISTS join_request_answers (
    id BIGSERIAL PRIMARY KEY,
    join_request_id BIGINT NOT NULL,
    community_question_id BIGINT NOT NULL,
    answer_text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_join_request_answers_join_request_id
        FOREIGN KEY (join_request_id) REFERENCES join_requests(id) ON DELETE CASCADE,
    CONSTRAINT fk_join_request_answers_community_question_id
        FOREIGN KEY (community_question_id) REFERENCES community_questions(id)
);

-- Index on join_request_id for fetching all answers for a request
CREATE INDEX IF NOT EXISTS idx_join_request_answers_join_request_id
    ON join_request_answers (join_request_id);
