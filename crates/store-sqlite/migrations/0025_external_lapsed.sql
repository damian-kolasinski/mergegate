-- A bot's comment that awaited the operator's vote lapses once a later
-- revision's diff no longer shows its lines: it stops holding the batch
-- (pending_at is cleared), and a vote up holds it again.
ALTER TABLE external_comments ADD COLUMN lapsed INTEGER NOT NULL DEFAULT 0
    CHECK (lapsed IN (0, 1));
