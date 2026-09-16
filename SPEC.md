# Technical specification: Remote machine management

Improve Vibe Kanban Admin / Settings Remote Access for paired remote machines,
independently of cluster workers. The current Host/Client landing page hides
existing connections, and the client list is conditional on available pairing
candidates. Make existing machines directly discoverable and manageable even
when hosts are offline or no new host can be paired.

Reuse relay pairing, identity, authorization, removal and host navigation APIs.
Show paired machines with name, identity, online/offline/unpaired status and
pairing date. Provide explicit keyboard-accessible Open workspaces and Remove
controls, refresh and distinct loading/error/empty states. Keep host setup and
client pairing reachable without obscuring the inventory. Removal must explain
its pairing scope and require confirmation; it must remain possible offline.
Disable workspace navigation unless the host is online. Do not equate failed
status discovery with a successful empty inventory.

Preserve local and remote frontend support, initial-host pairing deep links,
and current host navigation behavior. Use existing design tokens and translated
copy. Verify status/error and offline-management regressions, then run repository
format/check/lint and independent Codex review. No clustering, power controls,
SSH management, other-service changes, or deployment changes are included.

Clarified default scope: inventory, status, open and remove. Existing Host
display-name configuration remains available; no new rename API is planned.
