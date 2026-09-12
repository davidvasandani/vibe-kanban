-- Record the OS process group id of a browser session's spawned Chromium
-- process group so orphaned groups left behind by a server crash/SIGKILL can
-- be cleaned up at the next boot, mirroring execution_processes.pgid.
ALTER TABLE browser_sessions ADD COLUMN pgid INTEGER;
