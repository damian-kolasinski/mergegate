-- How urgent an open item is and why (F10): set on its first submit, changed
-- by the operator after. NULL priority sorts after p3.
ALTER TABLE items ADD COLUMN priority TEXT CHECK (priority IN ('p0', 'p1', 'p2', 'p3'));
ALTER TABLE items ADD COLUMN note TEXT;
