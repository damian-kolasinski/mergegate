-- The judge (design §7.4): each project's, and the verdicts it gave.

-- No row is no judge. `key` names what judged, as the domain derives it from
-- the provider and its model or command: a revision is judged again only
-- when it changes. The threshold and the order are applied on read, so
-- they aren't in the key.
CREATE TABLE judges (
    project_id INTEGER PRIMARY KEY REFERENCES projects (id),
    provider   TEXT NOT NULL CHECK (provider IN ('jev', 'command')),
    model      TEXT,
    command    TEXT,
    threshold  REAL NOT NULL CHECK (threshold > 0 AND threshold <= 1),
    ordering   TEXT NOT NULL CHECK (ordering IN ('guide', 'judge')),
    key        TEXT NOT NULL,
    CHECK ((provider = 'jev') = (model IS NOT NULL)
           AND (provider = 'command') = (command IS NOT NULL))
) STRICT;

-- A revision a judge judged, under that judge's key; judging it again
-- replaces the row and its files.
CREATE TABLE judged (
    item_id   INTEGER NOT NULL,
    revision  INTEGER NOT NULL,
    judged_by TEXT NOT NULL,
    PRIMARY KEY (item_id, revision),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;

-- One row per file the judge scored, which may be none of them. `promote` is
-- its confidence that a trusted file needs a look anyway.
CREATE TABLE judged_files (
    item_id    INTEGER NOT NULL,
    revision   INTEGER NOT NULL,
    path       TEXT NOT NULL,
    importance INTEGER NOT NULL CHECK (importance BETWEEN 1 AND 100),
    reason     TEXT,
    promote    REAL CHECK (promote BETWEEN 0 AND 1),
    PRIMARY KEY (item_id, revision, path),
    FOREIGN KEY (item_id, revision) REFERENCES judged (item_id, revision)
        ON DELETE CASCADE
) STRICT, WITHOUT ROWID;
