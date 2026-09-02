# Implementation Plan: Three rollout loose ends

Task: `vk/94c0-three-loose-ends`

1. Establish the SpecKit constitution baseline and produce the task-scoped
   specification, clarification, plan/research/contracts, tasks, and analysis
   artifacts required by stages 4–9.
2. Install the locked frontend dependencies required by repository checks.
3. Inspect all locale key sets and plural conventions, add accurate
   `metricsDiskAlerts` translations with unchanged interpolation tokens, and
   make the i18n key comparison use one explicit bytewise ordering contract.
4. Add a focused shell regression fixture for unsorted/nonnative input if the
   current script structure supports it cleanly; otherwise prove the helper's
   sorted output and exercise the full gate without diagnostics.
5. Add `start_background_helper_error_message`, route both direct validation and
   shared-preparation rejections through
   `ApiResponse::error_with_data_and_message`, and add an envelope-level test for
   every error variant.
6. Audit `error_with_data` usage reachable from MCP tools and record any further
   message-less boundaries without expanding this change beyond the requested
   helper fix.
7. Trace `include_apply_patch_tool` through repository history, settings schemas,
   and the pinned Codex 0.144.1 source/CLI. Remove the emitted dead key and any
   truly dead public setting only when compatibility permits.
8. Verify whether pinned `codex app-server` accepts `--strict-config`; if so,
   add it to the command builder and pin it with a command-level regression test.
   If not, implement and document the strongest verified fail-loud alternative.
9. Execute SpecKit's dependency-ordered task list, marking each item complete as
   its change and focused tests land.
10. Run formatting, the requested i18n reproduction, focused Rust tests, and
    proportionate frontend/backend checks. Preserve evidence in the SpecKit
    verification record.
11. Run an independent Codex CLI review of the complete diff, address every
    confirmed significant finding, re-run affected verification, and repeat the
    review until clean.
12. Add or update durable `docs/knowledge-base` topic pages for reusable lessons,
    tag them `vk/94c0-three-loose-ends`, refresh `INDEX.md`, and commit the
    knowledge-base change.
13. Re-read the current base tip and constitution numbering, ensure the worktree
    contains only this task's changes, commit the implementation, push the task
    branch, open a pull request against `main`, wait for required CI, fix any
    failures, and merge the pull request.
