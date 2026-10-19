# Prior knowledge — Debug VK Load

Read-only search of vibe-kanban/wiki/INDEX.md, worker-execution-hardening.md and coordinator-nfs-load.md, plus source references.

- Worker pressure guard intentionally refuses new work at memory or IO PSI full avg60 >= 40%; replay lookup precedes admission. A prior think3 incident involved local reclaim thrash and stuck agents. Preserve this protection.
- Heartbeats expose admission_refusing. The scheduler penalizes refusing workers by 1000 rather than excluding them; sticky placement means a workspace can still target a refusing worker.
- NFS D-state waits can raise load while IO PSI stays low. Do not infer NFS saturation solely from this 41.6% PSI error.
- Per-execution cgroups bound memory; io.weight is advisory with the none scheduler. Avoid claiming weights solve pressure without runtime evidence.
- Verify node /health pressure and active executions, and inspect actual placement/dispatch paths before proposing remediation.

Sources: vibe-kanban/wiki/worker-execution-hardening.md (vk/f268-harden-cluster-w); vibe-kanban/wiki/coordinator-nfs-load.md (vk/78a5-analyze-and-redu, vk/b923-workspaces-loadi).
