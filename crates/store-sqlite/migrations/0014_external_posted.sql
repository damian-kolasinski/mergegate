-- When GitHub says each external comment was posted, in Unix milliseconds by
-- its clock, to the second: an expected bot counts as heard from only by a
-- comment posted since the newest push (design §6.1), not by one the hub
-- happened to see after it.
--
-- The default only satisfies ALTER TABLE; every row gets its own. One taken
-- in before now gets when the hub last saw it change, which is what counted
-- for it until now.
ALTER TABLE external_comments ADD COLUMN posted_at INTEGER NOT NULL DEFAULT 0;
UPDATE external_comments SET posted_at = seen_at;
