# `/speckit.constitution`: Claude Fable 5.1 and Claude Code Refresh

The Vibe Kanban constitution was reviewed before feature specification. The
current constitution (version 0.31.0) already governs this task completely; no
new principle is required.

The controlling principles are:

- **II — Test the contract:** add focused coverage for the model catalog,
  reasoning options, exact package pin, and version-sensitive safety facts.
- **III — Small, reversible steps:** limit changes to the Claude executor and
  its governing Vibe Kanban deployment prerequisites.
- **VI — Don't rebuild what shipped:** extend
  `default_discovered_options()` and the existing pin/tests rather than add a
  second catalog or dependency mechanism.
- **IX — External agent protocols are defensive contracts:** verify model and
  tool identifiers against the vendor and pinned executing artifact, preserve
  unknown-event tolerance, keep parameter-level controls at the parameter, and
  record the verification source/version.
- **XIV — Repository verification is worktree-safe:** install the locked pnpm
  graph before repository checks.
- **XXI — One convention per concept:** use the Claude executor's established
  model ID/effort mapping and pass selected IDs unchanged to Claude Code.

There are no constitution deviations. Because no principle is added, there is
no provisional principle number to reconcile before merge.
