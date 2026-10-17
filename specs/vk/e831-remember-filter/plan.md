# Technical plan

Use a dedicated `features/kanban/model/kanbanPreferences.ts` model for validation, portable defaults and precedence. A `useKanbanPreferences.ts` hook owns browser-persisted remembered settings keyed by authenticated user/project/view and reads explicit defaults through `useScratch(UI_PREFERENCES, id)`. User defaults use the user UUID (separate from legacy global scratch UUID 1); site defaults use reserved UUID 2. Only the kanban project/view preference map in these isolated records carries default data. Do not touch the legacy shared preference record.

Integrate the hook into `features/kanban/ui/KanbanContainer.tsx` and expose save/reset controls through `shared/dialogs/kanban/KanbanFiltersDialog.tsx`. Keep matching/sorting logic unchanged. Surface localStorage write failures; disable default saves until records load and while saving. Remote frontend must hide site-wide controls because its scratch implementation is browser storage. Keep search transient. Use React state keyed by scope to avoid leaking one user's choices into another.

Validation: pure model and hook tests for persistence/default resolution/user changes, targeted frontend typechecking/lint, full required format. Independent Codex review before wiki update and PR merge.

Constitution: II verified by regression tests; III reuse scratch and existing primitives; IV both entrypoints checked; LI one resolver. No deviations.
