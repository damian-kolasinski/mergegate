-- How long the operator was active on each revision's review screen, in
-- milliseconds, as the UI measured it. Written only while the item is open,
-- so a closed item's totals, which its closing event reports, stay put.
CREATE TABLE review_time (
    item_id  INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    ms       INTEGER NOT NULL CHECK (ms >= 0),
    PRIMARY KEY (item_id, revision),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT, WITHOUT ROWID;
