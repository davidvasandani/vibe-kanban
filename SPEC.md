# Three Vibe Kanban rollout loose ends

## Purpose

Close three independent defects discovered during the VK pollers rollout: restore
the frontend i18n gate, preserve actionable background-helper rejection messages
through the API envelope, and remove an inert Codex configuration control while
making future invalid Codex configuration fail loudly where supported.

## Scope

Only the Vibe Kanban source repository is in scope. No other homelab service or
deployment module will be changed.

### 1. Restore the i18n gate

- Add localized `metricsDiskAlerts` strings to `common.json` for `es`, `fr`,
  `ja`, `ko`, `zh-Hans`, and `zh-Hant`.
- Preserve `{{error}}`, `{{severity}}`, and `{{count}}` interpolation tokens.
- Preserve the locale files' plural-key conventions while ensuring their scalar
  key sets match English.
- Repair `scripts/check-i18n.sh` so both operands passed to `comm` are proven to
  use the same bytewise sort order and malformed JSON cannot masquerade as an
  empty key set.
- The documented reproduction command must exit successfully without `comm`
  ordering diagnostics.

### 2. Make background-helper rejections actionable

- Every `StartBackgroundHelperError` variant must map to a stable human-readable
  message that identifies the rejected input or limit and tells the caller how
  to correct it.
- `start_background_helper` must return both typed `error_data` and the message,
  including rejections produced by shared helper preparation.
- A contract test must assert the message survives on the `ApiResponse` envelope
  that non-browser/MCP clients consume; testing only the enum variant is not
  sufficient.
- Consider other `error_with_data` routes reachable from MCP tooling and record
  any related findings, but avoid unrelated broad refactoring.

### 3. Eliminate dead Codex configuration

- Use repository history and the pinned Codex 0.144.1 source/protocol to establish
  the intent and current status of `include_apply_patch_tool`.
- Remove the inert key from thread configuration unless a verified current
  equivalent exists. Preserve user-settings compatibility only if doing so has a
  concrete migration benefit; do not continue emitting an unrecognized control.
- Determine whether `app-server --strict-config` is supported by the pinned CLI
  and compatible with Vibe Kanban's configuration flow. If verified, enable it
  and test the exact command contract. If it cannot safely be enabled, implement
  the strongest locally verifiable fail-loud assertion and document the reason.
- Tests must detect recurrence of the dead key and misspelling/omission of any
  adopted strict-config control.

## Compatibility and safety

- Frontend translation keys and API error enum shapes remain backward compatible.
- Existing valid background-helper and poller behavior is unchanged.
- Existing supported Codex settings continue to serialize and launch as before,
  except that invalid/unknown Codex configuration should fail visibly rather than
  being silently ignored.
- No generated files are edited by hand.

## Acceptance criteria

1. `GITHUB_BASE_REF=main ./scripts/check-i18n.sh` exits 0 without sort warnings.
2. Focused tests prove all helper rejection variants produce corrective messages
   in the API response envelope.
3. The emitted Codex thread config contains no `include_apply_patch_tool` key.
4. The verified fail-loud Codex configuration mechanism is covered by a focused
   regression test.
5. Relevant formatting, frontend checks, and Rust tests pass.
6. SpecKit artifacts are internally consistent and pass constitution analysis.
7. An independent Codex review reports no significant findings.
8. Reusable lessons are recorded in the project knowledge base, its index is
   refreshed, and that knowledge-base update is committed.
9. A pull request targeting the base branch is opened, CI passes, and the pull
   request is merged.
