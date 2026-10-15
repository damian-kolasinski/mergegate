-- How many unchanged lines follow a text diff's last hunk, so the view can
-- tell a hunk that reaches the end of the file from one with more after it.
-- NULL for a diff cached before it was counted, and unless `kind` is 'text'.
ALTER TABLE diffs ADD COLUMN lines_after INTEGER;
