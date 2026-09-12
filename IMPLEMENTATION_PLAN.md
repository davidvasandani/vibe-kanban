# Implementation plan

1. Capture the current `Trigger homelab CD` and `Test` workflow contents so
   validation can prove they were not functionally or textually changed.
2. Delete `.github/workflows/trigger-fork-deploy.yml`, removing the obsolete
   push/manual workflow, its private repository target, and its required
   `DEPLOY_REPO_TOKEN` secret reference.
3. Replace the README's obsolete private CI/CD deployment description with the
   active homelab dispatch path and its SHA/ref payload contract.
4. Validate all remaining workflow YAML, search for stale private-deployment
   references, and verify the homelab and Test workflow invariants.
5. Run an independent Codex review of the complete diff, resolve every
   confirmed significant finding, and repeat validation and review until no
   significant findings remain.
