-- Indexes for the workspaces-sidebar read paths.
--
-- `workspaces.archived` had no index at all, yet it is the filter for every
-- query behind the sidebar: `Workspace::find_all_with_status` (which now filters
-- and limits in SQL rather than in Rust), plus the batched
-- `find_latest_for_workspaces`, `find_workspaces_with_running_dev_servers`,
-- `get_latest_for_workspaces` and `find_workspaces_with_unseen`. The column order
-- matches `WHERE archived = ? ORDER BY updated_at DESC` so the same index serves
-- the sort.
CREATE INDEX IF NOT EXISTS idx_workspaces_archived_updated_at
    ON workspaces (archived, updated_at DESC);

-- `find_workspaces_with_unseen` filters `seen = 0` on a table that grows one row
-- per agent turn, so it was a full scan that got slower over time. Partial, since
-- only unseen rows are ever queried this way.
CREATE INDEX IF NOT EXISTS idx_coding_agent_turns_unseen
    ON coding_agent_turns (seen) WHERE seen = 0;
