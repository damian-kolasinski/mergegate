-- Reviews the operator submitted, each on the revision it was written on.
CREATE TABLE reviews (
    id           INTEGER PRIMARY KEY,
    item_id      INTEGER NOT NULL,
    revision     INTEGER NOT NULL,
    verdict      TEXT NOT NULL CHECK (verdict IN ('comment', 'changes_requested')),
    summary      TEXT NOT NULL,
    submitted_at TEXT NOT NULL,
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;

-- What an item's agent has to react to. `cursor` orders the whole log and is
-- never reused, so an agent's cursor stays valid. Each event points at the
-- rows it reports, which never change once written.
CREATE TABLE events (
    cursor    INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id   INTEGER NOT NULL REFERENCES items (id),
    type      TEXT NOT NULL CHECK (type IN ('review.submitted')),
    review_id INTEGER REFERENCES reviews (id),
    CHECK ((type = 'review.submitted') = (review_id IS NOT NULL))
) STRICT;

CREATE INDEX events_item ON events (item_id, cursor);

-- The cursor of the newest event the item's agent acknowledged.
ALTER TABLE items ADD COLUMN delivered INTEGER NOT NULL DEFAULT 0;
