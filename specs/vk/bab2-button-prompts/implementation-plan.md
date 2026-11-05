# Implementation plan
1. Refresh constitution review; execute repository SpecKit command instructions in order. These commands name specs/vk/5c0e-browser-scrolling as their feature directory; that path contains tracked historical artifacts, so apply the commands to this task directory to preserve existing specs.
2. Add empty-composer prompt actions to packages/ui/src/components/SessionChatBox.tsx, preserving specialized modes and non-idle actions.
3. Pass the linked issue simple_id from SessionChatBoxContainer.tsx; extend handleSend with an optional explicit prompt, preserving draft guards and existing composition/cleanup.
4. Extend packages/remote-web/src/test/SessionChatBox.test.tsx with exact prompt, visibility, draft and missing-issue checks.
5. Install dependencies, format, type-check and lint frontends, run focused tests. Independently review with Codex CLI, resolve findings, record knowledge, commit and open/merge PR.
