-- When set, workspace setup git-excludes `/specs/vk/` so pipeline design
-- artifacts stay local to the worktree instead of being committed.
ALTER TABLE repos ADD COLUMN local_pipeline_artifacts INTEGER NOT NULL DEFAULT 0;
