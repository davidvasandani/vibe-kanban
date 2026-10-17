# Plan: Lint web-core and remote-web

Spec: `specs/vk/848f-lint-packages-we/spec.md`. Constitution: v0.45.0
(XLVIII, XIV, III, IV, VI).

## Approach

### 1. One shared rule set (FR-4, FR-8)
- New `eslint.frontend.cjs` at the repo root exports
  `createFrontendConfig({ project, ignorePatterns })`. Its body is local-web's
  current `.eslintrc.cjs`, moved as is. The only parameters are the tsconfig
  path and package-specific ignores.
- `packages/local-web/.eslintrc.cjs` becomes a three-line call into the
  factory, keeping `src/routeTree.gen.ts` ignored. Proof that nothing changed:
  `eslint --print-config` output for sample files hashes the same against the
  old config and the new one. (Already checked: `__root.tsx`, `App.tsx`,
  `Bootstrap.tsx`, `vite.config.ts`.)
- The config objects are inlined with `require`, not used through `extends`,
  so ESLint resolves plugins from each consuming package. That is why
  remote-web gets the same ESLint devDependencies web-core already declares.
  These are the same versions (research R2), and nothing new enters the
  dependency graph.

### 2. Package wiring (FR-1, FR-2, FR-5)
- `packages/web-core/.eslintrc.cjs` and `packages/remote-web/.eslintrc.cjs`
  call the factory. Remote-web also ignores `src/routeTree.gen.ts`.
- `packages/web-core/tsconfig.eslint.json` extends `tsconfig.json`, includes
  `src` and `*.config.ts`, and clears the `exclude` that hides test files
  from `tsc`. `packages/remote-web/tsconfig.eslint.json` does the same.
  Remote-web's main tsconfig already includes its tests, and the lint
  tsconfig adds its config files. Neither file changes what `tsc` checks.
- Each package gets a `lint` script with local-web's flags. The root gets
  `web-core:lint` and `remote-web:lint`, and both join `pnpm run lint`.

### 3. CI (FR-3)
`.github/workflows/test.yml` `frontend-checks`: add `core:lint` and
`remote:lint` to the `concurrently` names and the matching
`cd packages/<pkg> && npm run lint` commands. `--kill-others-on-fail` already
fails the job when any command fails.

### 4. Naming rule (clarification)
In the shared `check-file/filename-naming-convention` map, PascalCase now
applies to `src/**/!(use*).tsx`, and `src/**/use*.{ts,tsx}` is camelCase.
Local-web has no `use*.tsx`, so its result doesn't change. Rename
`settingsRegistry.tsx` to `SettingsRegistry.tsx` and update its two
importers.

### 5. Layer boundaries: moves that change only import paths
| From | To | Importers updated |
|---|---|---|
| `features/create-mode/model/createModeSeedStore.ts` | `shared/stores/createModeSeedStore.ts` | `shared/actions/index.ts`, `pages/workspaces/WorkspacesLayout.tsx` |
| `shared/components/CreateChatBoxContainer.tsx` | `features/create-mode/ui/CreateChatBoxContainer.tsx` | `pages/kanban/ProjectRightSidebarContainer.tsx`, `pages/workspaces/WorkspacesLayout.tsx` |
| `shared/components/CreateModeRepoPickerBar.tsx` | `features/create-mode/ui/CreateModeRepoPickerBar.tsx` | `remote-web/src/test/CreateModeRepoPickerBar.test.tsx` (and its `vi.mock` paths) |
| `shared/components/ui-new/containers/SharedAppLayout.tsx` | `pages/root/SharedAppLayout.tsx` | `local-web/src/routes/_app.tsx`. Its three `./` sibling imports become `@/shared/components/ui-new/containers/...` |

Same-feature `@/features/create-mode/...` imports in `CreateModeProvider.tsx`
and `useCreateModeState.ts` become relative (`./x`), the way the other 60
intra-feature imports are written.

### 6. Code findings
Unused variables (keep each value's current shape):
- `localApiTransport.ts` and `relayHostApi.ts`: drop the routing-only keys
  by copying the object and calling `delete`. The fields are optional, so a
  shallow copy minus those keys is exactly what the rest-destructure built.
  Drop the unused `_options` parameter.
- `useKanbanIssueComposerStore.ts` `removeComposer`: copy `byKey`, then
  `delete` the key.
- `WorkspacesMainContainer.tsx`: stop destructuring `isSessionsLoading`. The
  props are destructured without a rest spread, so it goes nowhere new.
- `useIssueMultiSelect.ts`: drop the unused `_checked` parameter.
- `dataChannelWebSocket.ts`: `handleError()` takes no argument, and its
  call site stops passing one.

Other findings:
- `useLocalStorageScratch.ts` `catch {}`: add a comment that explains why the
  error is ignored (`no-empty` accepts a commented block).
- `MarkdownPreview.tsx`: remove both directives. Type `rehypePlugins` as
  react-markdown's `Options['rehypePlugins']` element list rather than
  `any[]`. Drop the `jsx-a11y/alt-text` directive, which names a plugin that
  isn't installed.
- `remote-web/.../webrtc/connection.ts`: the switch over inbound
  data-channel messages has no case for `http_request` and `ws_open`. Those
  are requests this side sends and never receives. Add explicit cases that
  ignore them, which matches today's fall-through.
- `remote-web/.../webrtc/index.ts` is a barrel. Its one importer
  (`Bootstrap.tsx`) imports from `./transport` directly, and the barrel is
  deleted.

### 7. `react-hooks/exhaustive-deps` (FR-7)
For each site: the decision, and why firing behaviour is unchanged.

| Site | Fix | Why behaviour is unchanged |
|---|---|---|
| `useConversationHistory` 377, 414 | Move `patchWithKey`, `toExecutionProcessState`, `streamHistoricExecutionProcessEntries` and `loadSettledProcessEntries` to module scope (they close over imports only). Wrap `loadEntriesForHistoricExecutionProcess` in `useCallback([])`, because it reads only a ref, and list it as a dep. | Module functions have no identity to track. The `useCallback` identity never changes, so neither callback is recreated any more often. |
| `useConversationHistory` 685 | Same: `loadSettledProcessEntries` is now module-scoped. | The effect's deps don't change. |
| `useConversationVirtualizer` 224 | Add `scrollContainerRef`. | It's a `RefObject` from the caller's `useRef`, so it's stable. Sibling effects already list it. |
| `useConversationVirtualizer` 338 | Remove `virtualizer` from `scrollToBottom`'s deps. | The body doesn't use it. `useVirtualizer` returns one instance for the component's life. |
| `ConversationListContainer` 421 | Replace the `filteredEntries` state and the ref-reading `useMemo` with `conversationRows` state, set in the same flush that sets the ref. | `displayEntries` is a fresh array on every flush, so the memo recomputed on every flush and returned the ref's current rows. Setting state to those same rows gives the same value and identity at the same render. `filteredEntries` had no other reader. The scope reset's `setFilteredEntries([])` only re-ran the memo, which returned the same `prevRowsRef.current` array the state already holds, so it is dropped. |
| `ConversationListContainer` 797 | Remove `firstUnvirtualizedRowIndex` and `scrollToAbsoluteIndex`. | The body doesn't use them. The only effect is fewer identity changes for the imperative handle. |
| `SessionChatBoxContainer` 639 | Remove `send`. | Unused in the body. |
| `ChangesPanelContainer` 788 | Capture `topBandCandidatesRef.current` (a `Set`, never reassigned) in the effect, and clear that in cleanup. | It's the same `Set` object. |
| `CreateModeRepoPickerBar` 213, 238 | Add `queryClient`. | `useQueryClient()` returns the provider's single client. |
| `CreatePRDialog` 290 | Remove `config?.pr_auto_description_enabled`. | Unused in the body. |
| `StartReviewDialog` 142, `ResolveConflictsDialog` 209 | Add `t`. | `t` changes only when the language changes. Then the handler picks up the new language, where today it kept the stale one. No effect depends on these handlers. |
| `AssigneeSelectionDialog` 106 | `useMemo` the `?? []` fallback. | Same value. The fresh `[]` on every render now keeps one identity, so `selectedIds` stops changing on every render. |
| `McpSettingsSection` 467 | `useMemo` the `profiles` filter on `readModel?.profiles`. | Same value, stable identity. It feeds only `useCallback` handlers. |
| `PipelinesSettingsSection` 137, 142 | Hoist the `['machine','unselected']` literal to a module constant, and `useMemo` the `statuses` fallback. | The statuses effect returns early unless `isSuccess`, and react-query's `data` keeps its identity between fetches. The effect still re-runs when the data changes. |
| `ReposSettingsSection` 137 | `useMemo` `reposQueryKey` on `machineClient?.queryScopeKey`. | react-query hashes keys, so identity doesn't matter there. The handlers stop being recreated on every render. |
| `useAzureAttachments` 148 | Capture the `Map` in the effect. | The ref is never reassigned, so it's the same object. |
| `useRemoteCloudHosts` 115 | `useMemo` the `?? []` fallback. | Same value, stable identity. |
| `ActionsProvider` 237, `RemoteActionsProvider` 137 | Add `appNavigation`. | local-web provides a module singleton. remote-web provides a `useMemo` keyed on the host id, so the executor context now refreshes when the host changes, where today it kept the stale navigation. |
| `McpServerDialog` (hidden behind a directive) | Remove the directive. Add `initialForm` and `isCustom`, which derive only from `codec` and `initial`. Read `profiles` through a ref that updates on every render. | Re-seeding still happens only when `modal.visible`, `codec` or `initial` changes. If `profiles` arrives late, the open form is not wiped (constitution X). |
| `RemoteAppShell` 91 | `useMemo` the `organizations` fallback. | Same value, stable identity. The org-selection effect runs when the data changes, not on every render. |
| `RemoteNavbarContainer` 182 | Remove `location.pathname`. | The flags derived from it (`isOnProjectPage`, `isOnWorkspaceView`) are already deps. |

## Verification
- `pnpm run lint` exits 0. `pnpm run check`, `pnpm run format`, the
  web-core vitest suite and the remote-web vitest suite all pass.
- Negative proof: add an unused import to one file in each package, check
  that `npm run lint` fails, then revert.
- `eslint --print-config` for local-web is unchanged (§1).

## Constitution check
- XLVIII: one shared rule set, a tsconfig that covers tests, no package-wide
  disables, and a judgement for every hook site. ✔
- XIV: the new lint commands run from the locked dependency graph, after
  `pnpm install --frozen-lockfile`. ✔
- III/VI: reuses local-web's config instead of forking it. The moves change
  import paths only. ✔
- IV: the moves restore the layer contract (shared doesn't depend on
  features or pages). ✔
- X: the McpServerDialog fix keeps the provisional-state seeding. ✔
