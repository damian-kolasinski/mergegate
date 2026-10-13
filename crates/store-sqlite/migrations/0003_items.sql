-- Units of work submitted for review. `number` counts per project, as in
-- app#42. An item keeps the backend and target it was opened with.
CREATE TABLE items (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects (id),
    number     INTEGER NOT NULL,
    branch     TEXT NOT NULL,
    title      TEXT NOT NULL,
    state      TEXT NOT NULL CHECK (state IN (
                   'needs_review', 'changes_requested', 'merging', 'merged', 'abandoned')),
    backend    TEXT NOT NULL CHECK (backend IN ('github', 'local')),
    target     TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (project_id, number)
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
-- are NULL for binary files.
CREATE TABLE revision_files (
    item_id   INTEGER NOT NULL,
    revision  INTEGER NOT NULL,
    position  INTEGER NOT NULL,
    path      TEXT NOT NULL,
    old_path  TEXT,
    additions INTEGER,
    deletions INTEGER,
    PRIMARY KEY (item_id, revision, position),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;
