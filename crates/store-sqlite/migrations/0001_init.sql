-- Hub-wide settings and facts as key/value pairs.
CREATE TABLE hub_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
