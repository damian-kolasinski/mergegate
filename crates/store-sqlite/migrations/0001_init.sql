-- The baseline schema. Databases from before it were reset rather than
-- migrated; from here on, migrations only move forward.

-- Hub-wide settings and facts as key/value pairs.
CREATE TABLE hub_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- Repositories registered with the hub. `upstream` is the canonical root of
-- the operator's repo, so one repo can't be registered twice. `github` is the
-- owner/repo the GitHub backend publishes to; a project switched back to
-- local keeps it for the next switch.
CREATE TABLE projects (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL UNIQUE,
    backend  TEXT NOT NULL CHECK (backend IN ('github', 'local')),
    target   TEXT NOT NULL,
    upstream TEXT NOT NULL UNIQUE,
    github   TEXT,
    CHECK (backend = 'local' OR github IS NOT NULL)
) STRICT;

-- Units of work submitted for review. `number` counts per project, as in
-- app#42. An item keeps the backend and target it was opened with; a GitHub
-- item has its pull request from its first submit on. `delivered` is the
-- cursor of the newest event the item's agent acknowledged.
CREATE TABLE items (
    id          INTEGER PRIMARY KEY,
    project_id  INTEGER NOT NULL REFERENCES projects (id),
    number      INTEGER NOT NULL,
    branch      TEXT NOT NULL,
    title       TEXT NOT NULL,
    state       TEXT NOT NULL CHECK (state IN (
                    'needs_review', 'changes_requested', 'merging', 'merged', 'abandoned')),
    backend     TEXT NOT NULL CHECK (backend IN ('github', 'local')),
    target      TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    delivered   INTEGER NOT NULL DEFAULT 0,
    pull_number INTEGER CHECK (pull_number >= 1),
    pull_url    TEXT,
    UNIQUE (project_id, number),
    CHECK ((backend = 'github') = (pull_number IS NOT NULL)),
    CHECK ((pull_number IS NULL) = (pull_url IS NULL))
) STRICT;

-- A branch has at most one open item.
CREATE UNIQUE INDEX items_open_branch ON items (project_id, branch)
    WHERE state IN ('needs_review', 'changes_requested', 'merging');

-- One row per submit. `base` is the merge-base with the item's target.
CREATE TABLE revisions (
    item_id      INTEGER NOT NULL REFERENCES items (id),
    number       INTEGER NOT NULL,
    head         TEXT NOT NULL,
    base         TEXT NOT NULL,
    files        INTEGER NOT NULL,
    additions    INTEGER NOT NULL,
    deletions    INTEGER NOT NULL,
    submitted_at TEXT NOT NULL,
    PRIMARY KEY (item_id, number)
) STRICT;

-- The files a revision changes against its base, in path order. Line counts
-- are NULL for binary files, a blob ID on the side where the file doesn't
-- exist. A submodule's blob IDs are commits.
CREATE TABLE revision_files (
    item_id   INTEGER NOT NULL,
    revision  INTEGER NOT NULL,
    position  INTEGER NOT NULL,
    path      TEXT NOT NULL,
    old_path  TEXT,
    additions INTEGER,
    deletions INTEGER,
    old_blob  TEXT,
    new_blob  TEXT,
    PRIMARY KEY (item_id, revision, position),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;

-- Closing an item drops the diffs of its blob pairs that no open item's files
-- still show; this finds those files by pair.
CREATE INDEX revision_files_blobs
    ON revision_files (coalesce(old_blob, ''), coalesce(new_blob, ''));

-- Each blob pair's diff, computed once when it's first submitted. A missing
-- side is '' so the key can't hold NULLs. `hunks` is the body of a unified
-- diff with 3 lines of context, empty unless `kind` is 'text'; line counts
-- are NULL for a binary change.
CREATE TABLE diffs (
    old_blob  TEXT NOT NULL,
    new_blob  TEXT NOT NULL,
    kind      TEXT NOT NULL CHECK (kind IN ('text', 'binary', 'too_large')),
    additions INTEGER,
    deletions INTEGER,
    hunks     TEXT NOT NULL,
    PRIMARY KEY (old_blob, new_blob)
) STRICT, WITHOUT ROWID;

-- Files the reviewer marked viewed. A mark holds for the revision it was
-- made on; a new revision starts with none.
CREATE TABLE viewed (
    item_id  INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    path     TEXT NOT NULL,
    PRIMARY KEY (item_id, revision, path),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT, WITHOUT ROWID;

-- Reviews the operator submitted, each on the revision it was written on.
-- Approving a revision is a review too, sent with the merge.
CREATE TABLE reviews (
    id           INTEGER PRIMARY KEY,
    item_id      INTEGER NOT NULL,
    revision     INTEGER NOT NULL,
    verdict      TEXT NOT NULL CHECK (verdict IN ('comment', 'changes_requested', 'approved')),
    summary      TEXT NOT NULL,
    submitted_at TEXT NOT NULL,
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;

-- What an item's agent has to react to (design §6.3). `cursor` orders the
-- whole log and is never reused, so an agent's cursor stays valid. Each event
-- points at the rows it reports, which never change once written.
-- `revision` is the item's newest when the event was logged, `source` who
-- acted. `body` is an abandon's reason or a held merge's; `commit_id` the
-- merge commit, or the target commit a conflict was found against; `paths` the
-- conflicting paths, one per line.
CREATE TABLE events (
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

CREATE INDEX events_item ON events (item_id, cursor);

-- Line comments: drafts until a review sends them (`review_id` set), and
-- unchanged from then on. IDs are hub-wide and never reused, as agents name
-- comments by them.
CREATE TABLE comments (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id    INTEGER NOT NULL,
    revision   INTEGER NOT NULL,
    review_id  INTEGER REFERENCES reviews (id),
    path       TEXT NOT NULL,
    side       TEXT NOT NULL CHECK (side IN ('old', 'new')),
    start_line INTEGER NOT NULL CHECK (start_line >= 1),
    line       INTEGER NOT NULL CHECK (line >= start_line),
    body       TEXT NOT NULL,
    quote      TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;

CREATE INDEX comments_item ON comments (item_id, id);
CREATE INDEX comments_review ON comments (review_id);

-- Each sent comment's thread. The agent's replies carry a status and go out
-- as they're written; the operator's are drafts until a review sends them
-- (`review_id` set). `revision` is the item's newest when the reply was
-- written, so the next one addressed it.
CREATE TABLE replies (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    comment_id INTEGER NOT NULL REFERENCES comments (id),
    author     TEXT NOT NULL CHECK (author IN ('agent', 'operator')),
    status     TEXT CHECK (status IN ('addressed', 'rejected', 'question')),
    review_id  INTEGER REFERENCES reviews (id),
    revision   INTEGER NOT NULL CHECK (revision >= 1),
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((author = 'agent') = (status IS NOT NULL)),
    CHECK (author = 'operator' OR review_id IS NULL),
    -- Only an addressed reply may leave the why to the next revision.
    CHECK (body <> '' OR status = 'addressed')
) STRICT;

CREATE INDEX replies_comment ON replies (comment_id, id);

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
