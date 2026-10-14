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
