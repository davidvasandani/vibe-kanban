# Feature Specification: Close three VK rollout loose ends
+
+**Feature dir**: `specs/vk/94c0-three-loose-ends/`
+**Task**: `vk/94c0-three-loose-ends`
+**Status**: Draft
+
+## Summary
+
+Restore a trustworthy frontend localization gate, ensure rejected background
+helpers tell agent callers what to correct, and eliminate a silently ignored
+Codex option while adopting a verified fail-loud configuration boundary.
+
+## User Stories
+
+- As a frontend contributor, I want the localization check on `main` to be green
+  and deterministic so a frontend change is evaluated on its own merits.
+- As an agent spawning a background helper, I want a rejected request to name
+  the invalid input or exhausted limit and tell me what to change.
+- As a maintainer configuring Codex, I want unsupported configuration to fail
+  visibly so a reviewed safety control cannot be silently inert.
+
+## Functional Requirements
+
+- FR-1: Every supported non-English locale must define the English disk-alert
+  strings and preserve interpolation variables and locale plural conventions.
+- FR-2: The localization consistency check must compare normalized key sets with
+  one bytewise ordering convention and reject malformed or unreadable JSON.
+- FR-3: The localization gate must emit no `comm` ordering diagnostics.
+- FR-4: Every background-helper rejection must retain typed error data and carry
+  an actionable response message naming the failure and corrective action.
+- FR-5: Tests must assert the caller-visible response envelope, not only an enum.
+- FR-6: Vibe Kanban must stop emitting the unsupported
+  `include_apply_patch_tool` Codex thread-config key.
+- FR-7: Pinned Codex history/source must establish the old setting's intent and
+  verify any current replacement or strict configuration control.
+- FR-8: Existing successful helper, poller, valid Codex, and typed frontend
+  behavior must remain compatible.
+- FR-9: Further MCP-reachable message-less errors found in a bounded audit are
+  recorded for follow-up, not silently expanded into this task.
+
+## Out of Scope
+
+- Other services or homelab deployment changes.
+- Localization redesign, helper-limit changes, broad response refactoring, or a
+  Codex upgrade.
+
+## Acceptance Criteria
+
+- [ ] `GITHUB_BASE_REF=main ./scripts/check-i18n.sh` exits 0 without ordering
+      diagnostics.
+- [ ] Locale key sets and placeholders match the English disk-alert contract.
+- [ ] All helper rejection variants expose typed data and actionable messages.
+- [ ] Codex emits no `include_apply_patch_tool` key.
+- [ ] A verified strict/fail-loud Codex launch contract has focused coverage.
+- [ ] Formatting, relevant checks, independent review, knowledge-base update,
+      CI, and merge complete.
+
+## Open Questions
+
+None. Stage 6 established that pinned Codex 0.144.1 supports
+`app-server --strict-config`; repository history shows
+`include_apply_patch_tool` was copied from the older V1 protocol into the V2
+config map after that typed field disappeared; and every shipped locale already
+uses the repository's `_one`/`_other` convention.
