# Implementation plan — Remote machine management

1. Complete the active workspace SpecKit constitution, specify, clarify, plan,
   tasks and analyze commands; use workspace-root PRIOR_KNOWLEDGE.md.
2. In `packages/web-core/src/shared/dialogs/settings/settings/RemoteCloudHostsSettingsCard.tsx`,
   separate inventory from optional pairing UI. Render inventory first regardless
   of discovery candidates, with explicit accessible navigation/removal actions,
   named confirmation, date/identity/status and refresh/loading/error states.
3. In `RelaySettingsSection.tsx` in the same directory, show machine management
   on the initial Remote Access screen, retain Host/Client setup and support
   initial pairing target in both runtimes. Avoid duplicate inventories.
4. Reuse query definitions in `useRelayRemoteHostMutations.ts`; ensure settings
   queries use the correct runtime and expose failures without changing app-bar
   behavior. Discovery failure disables opening but preserves paired records.
5. Add matching settings locale copy and targeted UI regression tests for no
   discovery candidates, offline/remove, failures, status gating and navigation.
6. Install frozen dependencies, run tests, format, check and lint. Review all
   resulting diff changes and exclude incidental formatter churn.
7. Run independent Codex CLI review, address significant findings and recheck.
8. Record reusable knowledge with task tag, commit artifacts/code, open and
   merge PR(s) against the repositories' actual base branches.
