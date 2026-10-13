-- Repositories registered with the hub. `upstream` is the canonical root of
-- the operator's repo, so one repo can't be registered twice.
CREATE TABLE projects (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL UNIQUE,
    backend  TEXT NOT NULL CHECK (backend IN ('github', 'local')),
    target   TEXT NOT NULL,
    upstream TEXT NOT NULL UNIQUE
) STRICT;
