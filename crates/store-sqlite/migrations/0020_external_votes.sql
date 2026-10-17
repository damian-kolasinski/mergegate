-- A comment from GitHub reaches the agent only once the operator votes it
-- up: 'up' holds it for the next batch, 'down' keeps it from the agent, and
-- none yet holds the batch. Comments held when this runs wait for a vote too.
ALTER TABLE external_comments ADD COLUMN vote TEXT
    CHECK (vote IS NULL OR vote IN ('up', 'down'));
