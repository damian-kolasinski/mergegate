-- The agent's review guide for each revision (F11), as Markdown it sent;
-- NULL when it sent none. A revision without one takes the last one's.
ALTER TABLE revisions ADD COLUMN guide TEXT;
