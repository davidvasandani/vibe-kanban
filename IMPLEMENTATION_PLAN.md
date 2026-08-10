# Implementation Plan: Deploy Status Mobile Header

1. Inspect the system-information Rust model, generated TypeScript contract,
   user-system provider, mobile navbar component, and existing UI test setup.
2. Establish a server-owned deployment start timestamp and add it
   backward-compatibly to `UserSystemInfo`; regenerate shared TypeScript types.
3. Thread the Git SHA and deployment timestamp from `useUserSystem` into the
   mobile navbar without changing the desktop application rail contract.
4. Add a compact mobile-header status component that formats the short SHA and
   elapsed deployment age, refreshes the age at a bounded cadence, and degrades
   safely for missing/development metadata.
5. Add focused tests for elapsed-age formatting and mobile render behavior,
   including absent metadata and narrow-layout semantics.
6. Run formatting plus targeted frontend/backend checks and tests, then inspect
   the resulting diff for generated-file correctness and unintended changes.
7. Run the required independent Codex diff review, address confirmed findings,
   and repeat verification until no significant findings remain.
8. Record any reusable implementation knowledge in the Vibe Kanban project
   knowledge base, update its index, and commit that documentation.
9. Commit the completed Vibe Kanban change and merge the task branch into its
   base branch.
