-- Each blob's syntax highlighting under a grammar, computed in the background
-- for the files open items show. `version` is that of the pack the grammar
-- came from, so a replaced pack's spans are never read. `lines` has a line
-- per line of the blob, each ending in '\n' and holding its spans as
-- `start,len,tag` separated by spaces, columns in UTF-16 code units; it's
-- empty for a blob that wasn't highlighted, as one too large.
CREATE TABLE highlights (
    blob    TEXT NOT NULL,
    grammar TEXT NOT NULL,
    version TEXT NOT NULL,
    lines   TEXT NOT NULL,
    PRIMARY KEY (blob, grammar, version)
) STRICT, WITHOUT ROWID;

-- Removing or replacing a pack drops its grammars' rows.
CREATE INDEX highlights_grammar ON highlights (grammar, version);
