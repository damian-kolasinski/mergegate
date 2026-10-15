-- An interdiff tells a file the later revision has and the earlier didn't
-- (`added`), or deletes and the earlier had (`deleted`), from one it changes
-- again. Both carry the diff from the earlier revision, as `changed` does;
-- the recorded ones are told apart by the side that diff has no blob on.
CREATE TABLE interdiff_files_new (
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
    change        TEXT NOT NULL CHECK (change IN ('changed', 'added', 'deleted', 'unchanged',
                                                  'unseparated', 'dropped')),
    old_mode      TEXT CHECK (old_mode IN ('100644', '100755', '120000', '160000')),
    new_mode      TEXT CHECK (new_mode IN ('100644', '100755', '120000', '160000')),
    PRIMARY KEY (item_id, from_revision, to_revision, position),
    FOREIGN KEY (item_id, from_revision, to_revision)
        REFERENCES interdiffs (item_id, from_revision, to_revision)
) STRICT;

INSERT INTO interdiff_files_new
    (item_id, from_revision, to_revision, position, path, old_path, additions, deletions,
     old_blob, new_blob, change, old_mode, new_mode)
SELECT item_id, from_revision, to_revision, position, path, old_path, additions, deletions,
       old_blob, new_blob,
       CASE
           WHEN change = 'changed' AND old_blob IS NULL THEN 'added'
           WHEN change = 'changed' AND new_blob IS NULL THEN 'deleted'
           ELSE change
       END,
       old_mode, new_mode
FROM interdiff_files;

DROP TABLE interdiff_files;
ALTER TABLE interdiff_files_new RENAME TO interdiff_files;
CREATE INDEX interdiff_files_blobs
    ON interdiff_files (coalesce(old_blob, ''), coalesce(new_blob, ''));
