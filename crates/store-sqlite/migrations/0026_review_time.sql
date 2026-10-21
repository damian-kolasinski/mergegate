-- How long the operator was active on an item's review screen, in
-- milliseconds, as the UI measured it: per turn, which each review they send
-- ends, and per revision on screen. Written only while the item is in review,
-- so a closed item's totals, which its closing event reports, stay put.
CREATE TABLE review_time (
    item_id  INTEGER NOT NULL,
    turn     INTEGER NOT NULL CHECK (turn >= 1),
    revision INTEGER NOT NULL,
    ms       INTEGER NOT NULL CHECK (ms >= 0),
    PRIMARY KEY (item_id, turn, revision),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT, WITHOUT ROWID;
