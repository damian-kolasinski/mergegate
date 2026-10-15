-- Whether two open items of a project merge into each other cleanly, as of
-- their heads then (F9). Each project's rows are replaced together, so pairs
-- of closed items or old heads last only until the next try.
CREATE TABLE conflicts (
    item_id    INTEGER NOT NULL REFERENCES items (id),
    other_id   INTEGER NOT NULL REFERENCES items (id),
    item_head  TEXT NOT NULL,
    other_head TEXT NOT NULL,
    conflicts  INTEGER NOT NULL CHECK (conflicts IN (0, 1)),
    PRIMARY KEY (item_id, other_id)
) STRICT;
