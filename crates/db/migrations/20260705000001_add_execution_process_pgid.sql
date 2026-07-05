-- Record the OS process group id of spawned execution processes so that
-- orphaned process groups left behind by a server crash/SIGKILL can be
-- cleaned up at the next boot.
ALTER TABLE execution_processes ADD COLUMN pgid INTEGER;
