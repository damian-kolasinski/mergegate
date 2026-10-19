-- A bot's summary of a pull request or of its review is for the operator to
-- read: never held for their vote or sent to the agent. Ones held when this
-- runs are told apart the next time the hub reads their pull request.
ALTER TABLE external_comments ADD COLUMN summary INTEGER NOT NULL DEFAULT 0;
