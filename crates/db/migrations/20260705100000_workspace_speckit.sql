-- SpecKit (Spec-Driven Development) provisioning state.
--
-- `speckit_feature_key`: the feature key (the workspace's branch, verbatim,
-- captured at first provisioning) under which SpecKit artifacts live
-- (`specs/<feature_key>/`). Once set it is never re-derived, so renames of the
-- workspace branch cannot orphan artifacts on disk.
--
-- `speckit_host_repo_id`: which repo worktree hosts `specs/` + `.specify/`
-- for multi-repo workspaces. Persisted together with the feature key at first
-- provisioning.
ALTER TABLE workspaces ADD COLUMN speckit_feature_key TEXT;
ALTER TABLE workspaces ADD COLUMN speckit_host_repo_id BLOB;
