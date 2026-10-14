-- Revision files gain their blob IDs, which existing rows lack: items are
-- dropped rather than backfilled, as the database may be reset before the
-- hub is in daily use.
DELETE FROM revision_files;
DELETE FROM revisions;
DELETE FROM items;

-- NULL on the side where the file doesn't exist. A submodule's are commits.
ALTER TABLE revision_files ADD COLUMN old_blob TEXT;
ALTER TABLE revision_files ADD COLUMN new_blob TEXT;

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
