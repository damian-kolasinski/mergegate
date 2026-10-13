-- Files the reviewer marked viewed. A mark holds for the revision it was
-- made on; a new revision starts with none.
CREATE TABLE viewed (
    item_id  INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    path     TEXT NOT NULL,
    PRIMARY KEY (item_id, revision, path),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT, WITHOUT ROWID;
