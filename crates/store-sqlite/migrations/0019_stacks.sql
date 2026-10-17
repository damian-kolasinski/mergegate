-- Stacks (design §5.1): an item stacked on another, and the events that tell
-- it its parent moved on.

-- The item it's stacked on, from the revision that stacked it until one
-- moves it off. It stays once the parent closes.
ALTER TABLE items ADD COLUMN parent_id INTEGER REFERENCES items (id);

-- Stacks are linear: an item has at most one open child.
CREATE UNIQUE INDEX items_open_child ON items (parent_id)
    WHERE parent_id IS NOT NULL
      AND state IN ('needs_review', 'changes_requested', 'merging');

-- `parent.updated` and `parent.abandoned` name the parent and `upstream`, the
-- parent revision the item's commits are built on; `parent.updated` keeps
-- what to rebase onto in `commit_id`, and whether the parent merged.
CREATE TABLE events_new (
    cursor    INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id   INTEGER NOT NULL REFERENCES items (id),
    type      TEXT NOT NULL CHECK (type IN (
                  'review.submitted', 'item.abandoned', 'merge.held', 'merge.cancelled',
                  'merge.conflict', 'item.merged', 'external.comments', 'comment.override',
                  'parent.updated', 'parent.abandoned')),
    revision  INTEGER NOT NULL CHECK (revision >= 1),
    source    TEXT NOT NULL CHECK (source IN ('operator', 'agent', 'external')),
    review_id INTEGER REFERENCES reviews (id),
    reply_id  INTEGER REFERENCES replies (id),
    body      TEXT,
    commit_id TEXT,
    paths     TEXT,
    parent_id INTEGER REFERENCES items (id),
    upstream  TEXT,
    merged    INTEGER CHECK (merged IN (0, 1)),
    CHECK ((type = 'review.submitted') = (review_id IS NOT NULL)),
    CHECK ((type = 'comment.override') = (reply_id IS NOT NULL)),
    CHECK (type <> 'external.comments' OR source = 'external'),
    CHECK (source <> 'external' OR type IN ('external.comments', 'item.merged', 'item.abandoned')),
    CHECK ((type IN ('item.merged', 'merge.conflict', 'parent.updated'))
           = (commit_id IS NOT NULL)),
    CHECK ((type = 'merge.conflict') = (paths IS NOT NULL)),
    CHECK ((type IN ('parent.updated', 'parent.abandoned')) = (parent_id IS NOT NULL)),
    CHECK ((type IN ('parent.updated', 'parent.abandoned')) = (upstream IS NOT NULL)),
    CHECK ((type = 'parent.updated') = (merged IS NOT NULL)),
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
DELETE FROM sqlite_sequence WHERE name = 'events_new';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'events_new', seq FROM sqlite_sequence WHERE name = 'events';
DROP TABLE events;
ALTER TABLE events_new RENAME TO events;
CREATE INDEX events_item ON events (item_id, cursor);
