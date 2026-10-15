-- The latest date GitHub gives each external comment, in Unix milliseconds by
-- its clock, to the second: when it was posted or last edited, a review when
-- it was submitted. An expected bot counts as heard from only by a comment
-- GitHub dates since the newest push (design §6.1), not by one the hub
-- happened to see after it.
--
-- The default only satisfies ALTER TABLE; every row gets its own. One taken
-- in before now gets when the hub last saw it change, which is what counted
-- for it until now.
ALTER TABLE external_comments ADD COLUMN dated_at INTEGER NOT NULL DEFAULT 0;
UPDATE external_comments SET dated_at = seen_at;
