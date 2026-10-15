-- Tailnet devices the hub keeps (design §4): the operator's decision, and
-- when each was last refused. Keyed by Tailscale's stable node ID, so
-- renaming a device keeps its decision.
CREATE TABLE devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    owner TEXT NOT NULL,
    access TEXT CHECK (access IN ('allow', 'deny')),
    asked_at INTEGER
) STRICT;
