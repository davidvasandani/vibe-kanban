# Independent Codex review

- Command: `codex review --base origin/main`
- Codex CLI: 0.155.1 (deployed)

## Result

One finding. There are no correctness findings in the catalog, test, saved-selection path or docs.

- **[P1] Gate the new models on a compatible deployed Codex runtime**
  (`codex.rs` new entries). The deployed 0.155.1 rejects all four new IDs
  with `model_not_found`, and discovery advertises them unconditionally.
  Suggested fixes: resolve the deployment prerequisite before shipping, or
  withhold the entries on incompatible runtimes.

## Disposition

Confirmed. This is the ship gate already recorded in `spec.md` (open
question) and `clarifications.md` Q5, and it is not fixable within this
task's scope:

- Upgrading the runtime means changing homelab `pkgs.codex`, which the task
  excludes.
- Withholding the entries by runtime version would leave the dropdown out of
  step with the selector on the current deployment, contradicting the
  acceptance criteria. It would also add version-probing plumbing (principle
  III).

The PR is therefore opened but **not merged** until the operator decides.
Code and docs needed no changes, so no re-review iteration was required.
