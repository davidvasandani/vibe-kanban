# Implementation plan
Task: vk/c817-aws-sso-sign-in

1. Recall project knowledge and trace coordinator AWS storage, worker scoped homes, shared CLI installation, and service PATH.
2. Refresh task-owned SpecKit commands, review constitution, specify behavior, resolve questions, and write a file-grounded technical plan and tasks; analyze before implementation.
3. Expose AWS state through the Vibe Kanban deployment's shared service storage and existing worker home overlay, preserving live login/refresh state and service-user permissions.
4. Ensure AWS CLI is in the worker toolchain; describe machine scope honestly in CLI Tools and AWS Settings, including remote-worker limitations.
5. Add regression checks for the actual boundaries, run relevant Rust/frontend/Nix verification and required setup/format checks.
6. Run independent Codex diff review, address confirmed findings, and re-verify.
7. Record reusable knowledge with task identity and commit it. Open and merge scoped PRs against each repository's base branch only after review and verification.

## Follow-up: status probe timeouts
1. Amend the existing spec using the live single/unbounded/bounded host results.
2. Add a shared four-permit probe gate in `crates/services/src/services/aws_sso.rs`; separate 30-second admission waiting from the 15-second execution budget.
3. Preserve ordered list results and existing credential sanitization. Test overlapping batches, queue/execution distinction, cancellation and classification.
4. Run focused services tests, required setup/format and relevant backend checks; independently review the diff.
5. Update project knowledge and task evidence, commit, then open and merge a scoped PR. Verify deployed Settings API statuses after rollout.
