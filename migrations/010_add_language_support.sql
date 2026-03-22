-- Add language support to applicant_sessions and community_questions
-- Supports English (en) and Ukrainian (uk) languages

-- Add language column to applicant_sessions
ALTER TABLE applicant_sessions
    ADD COLUMN IF NOT EXISTS language VARCHAR(5) NOT NULL DEFAULT 'en';

-- Add CHECK constraint to enforce valid language values
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE table_name = 'applicant_sessions'
        AND constraint_name = 'chk_applicant_sessions_language'
    ) THEN
        ALTER TABLE applicant_sessions
            ADD CONSTRAINT chk_applicant_sessions_language
            CHECK (language IN ('en', 'uk'));
    END IF;
END $$;

-- Index on language for efficient filtering by language
CREATE INDEX IF NOT EXISTS idx_applicant_sessions_language
    ON applicant_sessions (language);

-- Add Ukrainian question text column to community_questions
-- Default empty string - will be populated via config sync
ALTER TABLE community_questions
    ADD COLUMN IF NOT EXISTS question_text_uk TEXT NOT NULL DEFAULT '';
