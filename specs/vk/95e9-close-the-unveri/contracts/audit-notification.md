# Contract: Slack pinned-launcher audit notification

## Trigger

The real network test
`slack_pinned_launcher_matches_recorded_digest` exits non-zero during the daily
scheduled workflow (or a manual dispatch).

## Permissions

- `contents: read`
- `issues: write`

No user-supplied token or third-party webhook is required.

## Behaviour

1. The digest test remains the failing step and therefore the job remains red.
2. A following `if: failure()` notification step runs.
3. The step searches open issues for the exact incident title.
4. If found, it comments with:
   - the failed workflow name;
   - repository, run ID, and run URL;
   - a concise statement that the published launcher may have changed.
5. If not found, it creates an issue containing:
   - the same run evidence;
   - the affected release URL;
   - the expected recorded digest;
   - the instruction not to re-upload the existing tag;
   - the new-tag remediation rule.

## Failure semantics

- Notification failure does not mask or replace the digest-test failure.
- GitHub Actions' own failed-run signal remains the fallback.
- The workflow does not auto-close the issue after a green run.
- Logs and issue content contain no Slack credentials.

