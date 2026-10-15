-- Each side's git mode (design §5): an exec bit, a symlink or a submodule
-- changes a file without changing a line, so its blob pair alone can't show
-- it. NULL where the side is missing, and on both sides of a file recorded
-- before the hub kept modes.

ALTER TABLE revision_files ADD COLUMN old_mode TEXT
    CHECK (old_mode IN ('100644', '100755', '120000', '160000'));
ALTER TABLE revision_files ADD COLUMN new_mode TEXT
    CHECK (new_mode IN ('100644', '100755', '120000', '160000'));

ALTER TABLE interdiff_files ADD COLUMN old_mode TEXT
    CHECK (old_mode IN ('100644', '100755', '120000', '160000'));
ALTER TABLE interdiff_files ADD COLUMN new_mode TEXT
    CHECK (new_mode IN ('100644', '100755', '120000', '160000'));
