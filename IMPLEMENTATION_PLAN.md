# Implementation plan: verified Slack MCP delivery

Task: `95e9-close-the-unveri`

This plan builds on `SPEC.md`, `PRIOR_KNOWLEDGE.md`, and predecessor commit
`2e4b77aa`.

## 1. Establish the predecessor baseline

1. Confirm the working tree contains only this task's pipeline artefacts.
2. Incorporate predecessor commit `2e4b77aa` without discarding task-local
   files.
3. Resolve its root pipeline-document conflicts in favour of this task while
   retaining all predecessor implementation, specification, and knowledge-base
   files.
4. Run the focused predecessor Slack catalog tests to establish a green
   baseline.

## 2. Research the preventative delivery decision

1. Check whether a fork-controlled npm package already exists and whether the
   current environment is authenticated with authority to publish it.
2. Inspect npm's current registry metadata/integrity semantics from primary
   documentation and a registry response.
3. Inspect the managed CLI tool catalog's API, UI, PATH propagation, stable
   path, supported platform, and installation lifecycle.
4. Enumerate the exact repository and user-flow changes needed to make the
   Slack MCP entry depend on a managed executable.
5. Compare the options against the constitution and acceptance criteria:
   prevention strength, deployability now, clean-user behavior, platform
   coverage, operating burden, and rollback.
6. Choose npm delivery only if namespace ownership and publish authority are
   proven. Choose managed installation only if its missing-install lifecycle
   can be made explicit without silently breaking the preconfigured catalog.
   Otherwise write a formal detect-only decision with a concrete reopening
   trigger.

## 3. Specify contracts and tests

1. Define the canonical catalog command and the expected Codex and Opencode
   adaptations.
2. Define the source-of-truth version and integrity metadata.
3. Define Renovate matching and reviewer instructions for the chosen source.
4. Define clean-cache end-to-end setup and expected MCP results.
5. Define the audit-failure notification path or installer failure behavior.

## 4. Implement the chosen posture

### If npm registry delivery is available

1. Publish or verify the already-published fork launcher package at an exact
   immutable version outside this repository.
2. Confirm its packument has `dist.integrity` and its package contents match the
   reviewed launcher.
3. Replace the Slack URL package spec with exact `name@version`.
4. Replace GitHub-tarball digest constants/tests with checks appropriate to the
   npm package and recorded package integrity.
5. Update integration documentation, fork packaging knowledge, and Renovate to
   track the npm source.

### If managed installation is selected

1. Add the Slack MCP executable as a versioned, per-platform, SHA-256-pinned
   managed artefact.
2. Preserve staged download, verification-before-extraction/exec, atomic
   promotion, and stable executable exposure.
3. Add status/install/remove/API/UI support needed for the explicit per-user
   installation step.
4. Make the generated MCP entry resolve the stable managed executable and give
   an actionable failure when absent or stale.
5. Test unsupported platforms, checksum mismatch, clean install, repeated
   install, configuration adaptation, and removal.
6. Update integration documentation, fork packaging knowledge, and Renovate.

### If detection-only is accepted

1. Add a decision record to the fork packaging knowledge page, tagged with this
   task.
2. State why npm publication is unavailable and why a managed install is not
   adopted now.
3. State the exact residual threat, maximum detection window, notification
   owner/mechanism, and reopening trigger.
4. Tighten the digest audit to daily and add explicit workflow failure
   notification if a repository-native notification target exists and can be
   configured without inventing credentials.
5. Ensure user-facing integration documentation describes the delivery and
   verification posture accurately.

## 5. Verify

1. Install workspace dependencies with `pnpm install --frozen-lockfile`.
2. Run focused Rust tests for the Slack catalog, adapter shapes, and installer
   logic (if changed).
3. Run the ignored outer-artifact integrity test if still applicable.
4. Validate `renovate.json`.
5. Run a clean-cache stdio handshake and confirm `attachment_get_data` appears
   in `tools/list`.
6. Retrieve a real attachment when credentials and a fixture are present;
   otherwise record the precise external prerequisite after proving handler
   registration.
7. Run `pnpm run format`, relevant checks, and inspect the final diff.

## 6. Independent review and knowledge capture

1. Run the required independent Codex review against the complete diff.
2. Address every confirmed significant finding, rerun affected verification,
   and repeat review until no significant findings remain.
3. Update the relevant project knowledge page with reusable results, tag it
   `95e9-close-the-unveri`, refresh `docs/knowledge-base/INDEX.md`, and commit
   the knowledge-base change before handoff.
