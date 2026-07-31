-- Cleaning is an atomic dispatch fence: once claimed, no new execution can
-- start while coordinator-owned worktree reclamation is in progress.
ALTER TABLE workspaces
  ADD COLUMN placement_state_new TEXT NOT NULL DEFAULT 'local'
    CHECK (placement_state_new IN (
      'local', 'reserved', 'provisioning', 'ready', 'failed', 'cleaning'
    ));

UPDATE workspaces SET placement_state_new = placement_state;
DROP INDEX IF EXISTS idx_workspaces_placement_state;
ALTER TABLE workspaces DROP COLUMN placement_state;
ALTER TABLE workspaces RENAME COLUMN placement_state_new TO placement_state;
CREATE INDEX idx_workspaces_placement_state ON workspaces(placement_state);
