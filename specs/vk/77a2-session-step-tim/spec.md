# Feature specification: trustworthy session step times

Status: specified. Feature directory: `specs/vk/77a2-session-step-tim`.

As a user monitoring a long session, I want each step's recorded time so I can distinguish recent progress from earlier activity.

- FR-1: New live steps without vendor timestamps receive a server observation time, stable across updates and normal completed-history reads.
- FR-2: Valid source timestamps take precedence over observation times.
- FR-3: Historical replay never manufactures timestamps from the current clock. Legacy unknown step times are omitted, including aggregate members; synthetic prompt/script rows retain their explicit process time.
- FR-4: No changes to ordering, keys, timezone formatting, vendor protocols or other services.

Acceptance: two newly observed steps at different times have distinct times; replacement retains the first time; explicit source time wins; removal/reuse does not inherit stale time; replay without event time stays unknown; completed normalized sidecars preserve recorded times; frontend never displays process start as an unknown step's time.

Clarification resolved: use first observation at live normalization, not a guessed
vendor occurrence time. Historical/re-attached streams remain unstamped because
they can contain delayed replay. If a normalized sidecar is unavailable after a
restart, legacy replay may omit times; it must never fabricate current times.
No user decision is needed: this is constrained by the available evidence.

SpecKit commands are applied from the checked-in Markdown instructions. Their
stale feature/service paths are scoped to Vibe Kanban using the current workspace
feature pointer (`specs/vk/77a2-session-step-tim`), preserving other task specs.

Out of scope: reconstructing missing historical times, reworking vendor log formats, or repairing unrelated stuck execution lifecycle.
