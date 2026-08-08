ALTER TABLE workspace_affinity_operations
    ADD COLUMN run_on_coordinator INTEGER NOT NULL DEFAULT 0
        CHECK (run_on_coordinator IN (0, 1));
