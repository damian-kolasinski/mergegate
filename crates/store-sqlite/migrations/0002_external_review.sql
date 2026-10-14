-- External review (roadmap PR 10): comments from a GitHub pull request, the
-- batches that carry them to the agent, and the operator's override of an
-- agent's rejection.

-- The logins of the bots a github project expects a review from, separated
-- by commas; empty when it expects none.
ALTER TABLE projects ADD COLUMN bots TEXT NOT NULL DEFAULT '';

-- Comments gain a source. The operator's are line comments as before;
-- external ones are sent from the start (no review carries them) and may sit
-- on the item as a whole, with no lines.
CREATE TABLE comments_new (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id    INTEGER NOT NULL,
    revision   INTEGER NOT NULL,
    source     TEXT NOT NULL CHECK (source IN ('operator', 'external')),
    review_id  INTEGER REFERENCES reviews (id),
    path       TEXT,
    side       TEXT CHECK (side IN ('old', 'new')),
    start_line INTEGER CHECK (start_line >= 1),
    line       INTEGER CHECK (line >= start_line),
    body       TEXT NOT NULL,
    quote      TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number),
    CHECK ((path IS NULL) = (side IS NULL) AND (path IS NULL) = (start_line IS NULL)
           AND (path IS NULL) = (line IS NULL) AND (path IS NULL) = (quote IS NULL)),
    CHECK (source = 'external' OR path IS NOT NULL),
    CHECK (source = 'operator' OR review_id IS NULL)
) STRICT;

INSERT INTO comments_new
    (id, item_id, revision, source, review_id, path, side, start_line, line, body, quote,
     created_at)
SELECT id, item_id, revision, 'operator', review_id, path, side, start_line, line, body, quote,
       created_at
FROM comments;

-- Carries the high-water mark over, so a deleted draft's ID isn't reused.
DELETE FROM sqlite_sequence WHERE name = 'comments_new';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'comments_new', seq FROM sqlite_sequence WHERE name = 'comments';

DROP TABLE comments;
ALTER TABLE comments_new RENAME TO comments;
CREATE INDEX comments_item ON comments (item_id, id);
CREATE INDEX comments_review ON comments (review_id);

-- Replies gain the operator's override of an agent's rejection, which goes
-- out on its own (no review sends it) and may leave the why unsaid.
CREATE TABLE replies_new (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    comment_id INTEGER NOT NULL REFERENCES comments (id),
    author     TEXT NOT NULL CHECK (author IN ('agent', 'operator')),
    status     TEXT CHECK (status IN ('addressed', 'rejected', 'question')),
    review_id  INTEGER REFERENCES reviews (id),
    revision   INTEGER NOT NULL CHECK (revision >= 1),
    body       TEXT NOT NULL,
    override   INTEGER NOT NULL DEFAULT 0 CHECK (override IN (0, 1)),
    created_at TEXT NOT NULL,
    CHECK ((author = 'agent') = (status IS NOT NULL)),
    CHECK (author = 'operator' OR review_id IS NULL),
    CHECK (override = 0 OR (author = 'operator' AND review_id IS NULL)),
    CHECK (body <> '' OR status = 'addressed' OR override = 1)
) STRICT;

INSERT INTO replies_new
    (id, comment_id, author, status, review_id, revision, body, created_at)
SELECT id, comment_id, author, status, review_id, revision, body, created_at
FROM replies;

-- Carries the high-water mark over, so a deleted draft's ID isn't reused.
DELETE FROM sqlite_sequence WHERE name = 'replies_new';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'replies_new', seq FROM sqlite_sequence WHERE name = 'replies';

DROP TABLE replies;
ALTER TABLE replies_new RENAME TO replies;
CREATE INDEX replies_comment ON replies (comment_id, id);

-- Events gain the external source, a batch of external comments and the
-- operator's override, whose reply `reply_id` is.
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
    CHECK ((type = 'external.comments') = (source = 'external')),
    CHECK ((type IN ('item.merged', 'merge.conflict')) = (commit_id IS NOT NULL)),
    CHECK ((type = 'merge.conflict') = (paths IS NOT NULL)),
    CHECK (CASE type
               WHEN 'merge.held' THEN body IS NOT NULL
               WHEN 'item.abandoned' THEN 1
               ELSE body IS NULL
           END)
) STRICT;

INSERT INTO events_new
    (cursor, item_id, type, revision, source, review_id, body, commit_id, paths)
SELECT cursor, item_id, type, revision, source, review_id, body, commit_id, paths
FROM events;

-- Carries the high-water mark over, so a deleted draft's ID isn't reused.
DELETE FROM sqlite_sequence WHERE name = 'events_new';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'events_new', seq FROM sqlite_sequence WHERE name = 'events';

DROP TABLE events;
ALTER TABLE events_new RENAME TO events;
CREATE INDEX events_item ON events (item_id, cursor);

-- What GitHub says about an external comment. `seen_at` is when the hub
-- last saw it change, in Unix milliseconds by the hub's clock; `pending_at`
-- when it began waiting for a batch, NULL once one carried it.
-- `path` is the file an inline comment is on, which `comments` leaves out
-- when its lines couldn't be placed. `agreed_reply` is the agent's rejection
-- the operator agreed with.
CREATE TABLE external_comments (
    comment_id   INTEGER PRIMARY KEY REFERENCES comments (id),
    item_id      INTEGER NOT NULL REFERENCES items (id),
    kind         TEXT NOT NULL CHECK (kind IN ('review_comment', 'issue_comment', 'review')),
    github_id    INTEGER NOT NULL,
    login        TEXT NOT NULL,
    bot          INTEGER NOT NULL CHECK (bot IN (0, 1)),
    url          TEXT NOT NULL,
    path         TEXT,
    seen_at      INTEGER NOT NULL,
    pending_at   INTEGER,
    agreed_reply INTEGER REFERENCES replies (id),
    UNIQUE (item_id, kind, github_id)
) STRICT;

CREATE INDEX external_comments_pending ON external_comments (item_id)
    WHERE pending_at IS NOT NULL;

-- The comments an `external.comments` event carried, with each body as it
-- was then, so the event reads the same after an edit. `edited` marks one an
-- earlier batch carried already.
CREATE TABLE event_comments (
    cursor     INTEGER NOT NULL REFERENCES events (cursor),
    comment_id INTEGER NOT NULL REFERENCES comments (id),
    body       TEXT NOT NULL,
    edited     INTEGER NOT NULL CHECK (edited IN (0, 1)),
    PRIMARY KEY (cursor, comment_id)
) STRICT, WITHOUT ROWID;
