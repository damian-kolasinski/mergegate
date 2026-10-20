-- The tailnet device an item's agent submits from, whose `mg wakes` runner
-- runs its wake command. NULL for the hub's machine, where the hub runs it.
ALTER TABLE item_agents ADD COLUMN device TEXT;
