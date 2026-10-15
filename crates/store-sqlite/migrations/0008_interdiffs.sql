-- Interdiffs (F7, design §9.2): what a revision changes since each earlier
-- one of its item, recorded with it at submit. A row here with no files is an
-- interdiff that changes nothing; no row is one that wasn't recorded, as for
-- revisions submitted before this migration.
CREATE TABLE interdiffs (
    item_id       INTEGER NOT NULL,
    from_revision INTEGER NOT NULL,
    to_revision   INTEGER NOT NULL,
    PRIMARY KEY (item_id, from_revision, to_revision),
    FOREIGN KEY (item_id, from_revision) REFERENCES revisions (item_id, number),
    FOREIGN KEY (item_id, to_revision) REFERENCES revisions (item_id, number),
    CHECK (from_revision < to_revision)
) STRICT, WITHOUT ROWID;

-- An interdiff's files, in path order. `changed` and `dropped` files carry
-- the diff from the earlier revision, replayed onto the later one's base, to
-- the later revision; `unchanged` and `unseparated` ones the later revision's
-- own diff. Blobs and line counts are as in `revision_files`.
CREATE TABLE interdiff_files (
    item_id       INTEGER NOT NULL,
    from_revision INTEGER NOT NULL,
    to_revision   INTEGER NOT NULL,
    position      INTEGER NOT NULL,
    path          TEXT NOT NULL,
    old_path      TEXT,
    additions     INTEGER,
    deletions     INTEGER,
    old_blob      TEXT,
    new_blob      TEXT,
    change        TEXT NOT NULL CHECK (change IN ('changed', 'unchanged', 'unseparated', 'dropped')),
    PRIMARY KEY (item_id, from_revision, to_revision, position),
    FOREIGN KEY (item_id, from_revision, to_revision)
        REFERENCES interdiffs (item_id, from_revision, to_revision)
) STRICT;

-- Pruning diffs keeps the pairs an open item's interdiffs still show.
CREATE INDEX interdiff_files_blobs
    ON interdiff_files (coalesce(old_blob, ''), coalesce(new_blob, ''));
