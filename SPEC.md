# Workspace creation progress

Task: vk/855b-show-the-steps-w

## Problem and outcome
The workspace creation view currently shows a spinner with no explanation of the work taking place. Display actual creation steps and background activity while the existing asynchronous creation operation runs, including after navigation away and return.

## Requirements
- Show ordered creation steps, distinguishing waiting, running, completed, and failed states from backend evidence.
- Identify the current background work (repository preparation, workspace configuration, and agent startup as supported by the existing creation flow).
- Preserve request-independent creation, single-consumer claiming, existing error reporting, and ready-state navigation.
- Keep progress scoped to the workspace, readable in narrow layouts, and accessible.
- Do not expose shell output, credentials, or invented percentage completion.
- Scope changes to the Vibe Kanban repository; deployment changes only if required for this feature.

## Technical approach
Inspect the existing durable creation lifecycle and extend its observable state with bounded step information. Reuse existing authenticated workspace reads or a workspace-scoped progress endpoint, and existing frontend query conventions. Persist enough progress for reload/reconnect; terminal failure must stop any running indicators.

## Acceptance and verification
Exercise slow creation, navigation/reload, successful handoff, failure, and recovery after restart. Add focused backend lifecycle and frontend projection tests, regenerate any shared contracts, and run repository formatting and applicable checks. Complete SpecKit artifacts, independent Codex review, knowledge-base update, then open and merge the task PR.
