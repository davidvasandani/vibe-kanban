CREATE TABLE workspace_archive_events (
    workspace_id UUID PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    trigger_key UUID NOT NULL DEFAULT gen_random_uuid(),
    rule TEXT NOT NULL DEFAULT 'remote_archive',
    triggered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE FUNCTION record_workspace_archive_event() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.archived AND (TG_OP = 'INSERT' OR NOT OLD.archived) THEN
        INSERT INTO workspace_archive_events (workspace_id) VALUES (NEW.id)
        ON CONFLICT (workspace_id) DO UPDATE SET trigger_key = gen_random_uuid(), rule = 'remote_archive', triggered_at = NOW();
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER workspace_archive_event AFTER INSERT OR UPDATE OF archived ON workspaces
FOR EACH ROW EXECUTE FUNCTION record_workspace_archive_event();
INSERT INTO workspace_archive_events (workspace_id, rule, triggered_at)
SELECT id, 'legacy_remote_archive', updated_at FROM workspaces WHERE archived;
