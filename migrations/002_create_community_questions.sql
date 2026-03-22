-- Create community_questions table
CREATE TABLE IF NOT EXISTS community_questions (
    id BIGSERIAL PRIMARY KEY,
    community_id BIGINT NOT NULL,
    question_key TEXT NOT NULL,
    question_text TEXT NOT NULL,
    required BOOLEAN NOT NULL DEFAULT TRUE,
    position INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_community_questions_community_id
        FOREIGN KEY (community_id) REFERENCES communities(id) ON DELETE CASCADE
);

-- Unique constraint: no duplicate positions per community
CREATE UNIQUE INDEX IF NOT EXISTS uq_community_questions_community_position
    ON community_questions (community_id, position)
    WHERE is_active = TRUE;

-- Unique constraint: no duplicate keys per community
CREATE UNIQUE INDEX IF NOT EXISTS uq_community_questions_community_key
    ON community_questions (community_id, question_key)
    WHERE is_active = TRUE;
