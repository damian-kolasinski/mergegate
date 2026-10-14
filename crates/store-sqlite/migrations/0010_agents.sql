-- What an item's agent left with its newest submit that carried it: the
-- worktree it works in and the command that wakes it (design §6.5). `woken`
-- is the newest event the hub woke the agent for, so a restarted hub doesn't
-- wake it again for the same events.
CREATE TABLE item_agents (
    item_id  INTEGER PRIMARY KEY REFERENCES items (id),
    worktree TEXT,
    wake     TEXT CHECK (wake <> ''),
    woken    INTEGER NOT NULL DEFAULT 0
) STRICT;
