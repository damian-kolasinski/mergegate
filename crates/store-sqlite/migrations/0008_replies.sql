-- Each sent comment's thread. The agent's replies carry a status and go out
-- as they're written; the operator's are drafts until a review sends them
-- (`review_id` set). `revision` is the item's newest when the reply was
-- written, so the next one addressed it.
CREATE TABLE replies (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    comment_id INTEGER NOT NULL REFERENCES comments (id),
    author     TEXT NOT NULL CHECK (author IN ('agent', 'operator')),
    status     TEXT CHECK (status IN ('addressed', 'rejected', 'question')),
    review_id  INTEGER REFERENCES reviews (id),
    revision   INTEGER NOT NULL CHECK (revision >= 1),
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((author = 'agent') = (status IS NOT NULL)),
    CHECK (author = 'operator' OR review_id IS NULL),
    -- Only an addressed reply may leave the why to the next revision.
    CHECK (body <> '' OR status = 'addressed')
) STRICT;

CREATE INDEX replies_comment ON replies (comment_id, id);
