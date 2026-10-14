-- Changes to the rules that wait for the operator (design §7.3): rules
-- agents propose and hand edits that would hide more.

-- `applied_text` is the rules file as it was when its rules were last
-- applied, which rejecting an edit writes back; NULL while none was.
-- `last_rule` is the highest rule number the project ever used, so a new
-- rule never takes a retired one's ID.
CREATE TABLE rules_files (
    project_id   INTEGER PRIMARY KEY REFERENCES projects (id),
    applied_text TEXT,
    last_rule    INTEGER NOT NULL CHECK (last_rule >= 0)
) STRICT;

INSERT INTO rules_files (project_id, applied_text, last_rule)
    SELECT project_id, NULL, max(id) FROM applied_rules GROUP BY project_id;

-- A `review` proposal applied when it arrived and has no preview; a
-- `trusted` one waits and records how many items its preview looked at.
CREATE TABLE rule_proposals (
    project_id    INTEGER NOT NULL REFERENCES projects (id),
    id            INTEGER NOT NULL CHECK (id >= 1),
    tier          TEXT NOT NULL CHECK (tier IN ('review', 'trusted')),
    path          TEXT,
    kind          TEXT CHECK (kind IN ('added', 'deleted', 'renamed', 'modified', 'generated')),
    max_lines     INTEGER CHECK (max_lines >= 0),
    diff_matches  TEXT,
    reason        TEXT NOT NULL,
    state         TEXT NOT NULL CHECK (state IN ('pending', 'applied', 'rejected')),
    preview_items INTEGER CHECK (preview_items >= 0),
    PRIMARY KEY (project_id, id)
) STRICT;

-- The hand edit waiting for the operator, at most one per project, with
-- its rules in `held_rules` in file order.
CREATE TABLE held_edits (
    project_id    INTEGER PRIMARY KEY REFERENCES projects (id),
    text          TEXT NOT NULL,
    preview_items INTEGER NOT NULL CHECK (preview_items >= 0)
) STRICT;

CREATE TABLE held_rules (
    project_id   INTEGER NOT NULL REFERENCES held_edits (project_id) ON DELETE CASCADE,
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

-- The files a change's preview found it would hide, in the order found:
-- the proposal's when `proposal` is set, else the held edit's.
CREATE TABLE hidden_files (
    project_id INTEGER NOT NULL REFERENCES projects (id),
    proposal   INTEGER CHECK (proposal >= 1),
    item_id    INTEGER NOT NULL REFERENCES items (id),
    path       TEXT NOT NULL,
    rule       INTEGER NOT NULL CHECK (rule >= 1)
) STRICT;

CREATE INDEX hidden_files_change ON hidden_files (project_id, proposal);
