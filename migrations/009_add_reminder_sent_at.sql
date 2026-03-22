-- Add reminder_sent_at column to join_requests (needed for Task 7: reminder scheduling)
ALTER TABLE join_requests
    ADD COLUMN IF NOT EXISTS reminder_sent_at TIMESTAMPTZ;
