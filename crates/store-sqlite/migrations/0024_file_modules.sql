-- The module each file of a revision is in, found when the revision was
-- recorded: its folder, and the target folder within it the file is in.
-- NULL when no module holds the file, and for a file recorded before modules
-- were found.
ALTER TABLE revision_files ADD COLUMN module TEXT;
ALTER TABLE revision_files ADD COLUMN target TEXT;
