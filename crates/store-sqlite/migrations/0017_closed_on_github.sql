-- An item whose pull request was merged or closed on GitHub closes with an
-- `item.merged` or `item.abandoned` event GitHub is the source of, which
-- 0002's checks kept to `external.comments`. A batch of comments is still
-- only ever GitHub's.
CREATE TABLE events_new (
    cursor    INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id   INTEGER NOT NULL REFERENCES items (id),
    type      TEXT NOT NULL CHECK (type IN (
                  'review.submitted', 'item.abandoned', 'merge.held', 'merge.cancelled',
                  'merge.conflict', 'item.merged', 'external.comments', 'comment.override')),
    revision  INTEGER NOT NULL CHECK (revision >= 1),
    source    TEXT NOT NULL CHECK (source IN ('operator', 'agent', 'external')),
    review_id INTEGER REFERENCES reviews (id),
    reply_id  INTEGER REFERENCES replies (id),
    body      TEXT,
    commit_id TEXT,
    paths     TEXT,
    CHECK ((type = 'review.submitted') = (review_id IS NOT NULL)),
    CHECK ((type = 'comment.override') = (reply_id IS NOT NULL)),
    CHECK (type <> 'external.comments' OR source = 'external'),
    CHECK (source <> 'external' OR type IN ('external.comments', 'item.merged', 'item.abandoned')),
    CHECK ((type IN ('item.merged', 'merge.conflict')) = (commit_id IS NOT NULL)),
    CHECK ((type = 'merge.conflict') = (paths IS NOT NULL)),
    CHECK (CASE type
               WHEN 'merge.held' THEN body IS NOT NULL
               WHEN 'item.abandoned' THEN 1
               ELSE body IS NULL
           END)
) STRICT;

INSERT INTO events_new
    (cursor, item_id, type, revision, source, review_id, reply_id, body, commit_id, paths)
SELECT cursor, item_id, type, revision, source, review_id, reply_id, body, commit_id, paths
FROM events;

-- Carries the high-water mark over, so a deleted event's cursor isn't reused.
DELETE FROM sqlite_sequence WHERE name = 'events_new';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'events_new', seq FROM sqlite_sequence WHERE name = 'events';

DROP TABLE events;
ALTER TABLE events_new RENAME TO events;
CREATE INDEX events_item ON events (item_id, cursor);
