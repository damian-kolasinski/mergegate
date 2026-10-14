-- File triage (design §7). Files submitted before it are reviewed, as no
-- rule existed to trust them.

-- `generated` is `linguist-generated` as the revision's base sets it. A file
-- reviewed by default has no `rule` or `guard`; `why` is the matching rule's
-- matchers and reason as they were when it matched.
ALTER TABLE revision_files ADD COLUMN generated INTEGER NOT NULL DEFAULT 0
    CHECK (generated IN (0, 1));
ALTER TABLE revision_files ADD COLUMN tier TEXT NOT NULL DEFAULT 'review'
    CHECK (tier IN ('review', 'trusted'));
ALTER TABLE revision_files ADD COLUMN rule INTEGER CHECK (rule >= 1);
ALTER TABLE revision_files ADD COLUMN guard TEXT CHECK (guard IN ('gitattributes'));
ALTER TABLE revision_files ADD COLUMN why TEXT
    CHECK ((rule IS NULL) = (why IS NULL) AND (rule IS NULL OR guard IS NULL)
           AND (guard IS NULL OR tier = 'review'));

-- The rules each project's files were last triaged by, in order; `id` is the
-- number of R<n>.
CREATE TABLE applied_rules (
    project_id   INTEGER NOT NULL REFERENCES projects (id),
    position     INTEGER NOT NULL,
    id           INTEGER NOT NULL CHECK (id >= 1),
    tier         TEXT NOT NULL CHECK (tier IN ('review', 'trusted')),
    path         TEXT,
    kind         TEXT CHECK (kind IN ('added', 'deleted', 'renamed', 'modified', 'generated')),
    max_lines    INTEGER CHECK (max_lines >= 0),
    diff_matches TEXT,
    reason       TEXT NOT NULL,
    PRIMARY KEY (project_id, position),
    UNIQUE (project_id, id)
) STRICT;
