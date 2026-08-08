# Technical Plan

1. Add a pure async restart orchestration helper beside workspace-chat hooks.
2. For stopped sessions call the existing send callback with a fixed prompt.
3. For running sessions await confirmation, then queue that prompt unless user
   input is already queued.
4. In `SessionChatBoxContainer`, compute selected-session coding-agent running
   state, show `ConfirmDialog`, and surface queued/started toast feedback.
5. Cover orchestration branches with Vitest and run web-core checks.

This reuses shared lifecycle/UI boundaries and explicitly names the operation a
restart, satisfying the constitution without claiming live inventory adoption.
