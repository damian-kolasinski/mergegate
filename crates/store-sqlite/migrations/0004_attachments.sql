-- When an item merged or was abandoned; NULL while it's open. Items that
-- closed before this column existed count as closed now.
ALTER TABLE items ADD COLUMN closed_at TEXT;
UPDATE items SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    WHERE state IN ('merged', 'abandoned');

-- Demo media attached to a revision, in the order added. The file itself is
-- kept once per `sha256`, in the attachments folder next to the database.
CREATE TABLE attachments (
    item_id    INTEGER NOT NULL,
    revision   INTEGER NOT NULL,
    position   INTEGER NOT NULL,
    name       TEXT NOT NULL,
    media      TEXT NOT NULL,
    size       INTEGER NOT NULL CHECK (size >= 0),
    sha256     TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (item_id, revision, position),
    FOREIGN KEY (item_id, revision) REFERENCES revisions (item_id, number)
) STRICT;

CREATE INDEX attachments_sha256 ON attachments (sha256);
