-- Closing an item, and the merge on its way there, reach its agent as events
-- (design §6.3). SQLite can't alter a CHECK, so `events` is rebuilt; rows keep
-- their cursors, and the AUTOINCREMENT sequence is carried over.
-- `revision` is the item's newest when the event was logged, `source` who
-- acted. `body` is an abandon's reason or a held merge's; `commit_id` the
-- merge commit, or the target commit a conflict was found against; `paths` the
-- conflicting paths, one per line.
CREATE TABLE events_new (
    cursor    INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id   INTEGER NOT NULL REFERENCES items (id),
    type      TEXT NOT NULL CHECK (type IN (
                  'review.submitted', 'item.abandoned', 'merge.held', 'merge.cancelled',
                  'merge.conflict', 'item.merged')),
    revision  INTEGER NOT NULL CHECK (revision >= 1),
    source    TEXT NOT NULL CHECK (source IN ('operator', 'agent')),
    review_id INTEGER REFERENCES reviews (id),
    body      TEXT,
    commit_id TEXT,
    paths     TEXT,
    CHECK ((type = 'review.submitted') = (review_id IS NOT NULL)),
    CHECK ((type IN ('item.merged', 'merge.conflict')) = (commit_id IS NOT NULL)),
    CHECK ((type = 'merge.conflict') = (paths IS NOT NULL)),
    CHECK (CASE type
               WHEN 'merge.held' THEN body IS NOT NULL
               WHEN 'item.abandoned' THEN 1
               ELSE body IS NULL
           END)
) STRICT;

INSERT INTO events_new (cursor, item_id, type, revision, source, review_id)
SELECT e.cursor, e.item_id, e.type, r.revision, 'operator', e.review_id
FROM events e JOIN reviews r ON r.id = e.review_id;

-- The copy leaves the sequence at the newest cursor copied, which is lower
-- than the old one if the newest events were ever removed. The rename moves it.
DELETE FROM sqlite_sequence WHERE name = 'events_new';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'events_new', seq FROM sqlite_sequence WHERE name = 'events';

DROP TABLE events;
ALTER TABLE events_new RENAME TO events;
CREATE INDEX events_item ON events (item_id, cursor);

-- Approving a revision is a review too, sent with the merge (PR 7's second
-- half). Rebuilt here so the events above can point at the new table.
CREATE TABLE reviews_new (
    id           INTEGER PRIMARY KEY,
    item_id      INTEGER NOT NULL,
    revision     INTEGER NOT NULL,
    verdict      TEXT NOT NULL CHECK (verdict IN ('comment', 'changes_requested', 'approved')),
    summary      TEXT NOT NULL,
    submitted_at TEXT NOT NULL,
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;

INSERT INTO reviews_new SELECT * FROM reviews;
DROP TABLE reviews;
ALTER TABLE reviews_new RENAME TO reviews;

-- Closing an item drops the diffs of its blob pairs that no open item's files
-- still show; this finds those files by pair.
CREATE INDEX revision_files_blobs
    ON revision_files (coalesce(old_blob, ''), coalesce(new_blob, ''));
