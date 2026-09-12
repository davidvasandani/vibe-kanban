# Technical Plan

Add an explicit legacy-origin field to the shared MCP write representation or a
parallel migration map so the server returned under its safe identifier can
name the native key it replaces. Build migration candidates during snapshot
merge, reject collisions globally, and preserve labels through the existing
sidecar. Extend the write planner to remove the legacy key and insert the safe
key in each assigned profile after all profiles pass validation. Reuse existing
staged native writes and metadata recovery.

Validate through focused executor and server-route tests plus generated type
checks if the public contract changes.
