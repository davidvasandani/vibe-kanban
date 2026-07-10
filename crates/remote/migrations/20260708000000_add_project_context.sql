-- Freeform, cloud-synced "project context" briefing, injected into the agent
-- prompt for every issue spawned from the project. Empty string = no context
-- (no injection). Existing projects default to empty with no migration prompt.
ALTER TABLE projects ADD COLUMN IF NOT EXISTS context TEXT NOT NULL DEFAULT '';
