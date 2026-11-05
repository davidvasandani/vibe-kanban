# Session composer prompt actions

An empty (including whitespace) idle existing-session composer renders Continue,
What’s left and Status. These send `Continue working on this task`,
`What’s left on <simple_id>` and `Status` respectively. New-session, review
comment, queue, stop, approval, question and edit actions keep their existing
behavior. Keyboard submission still requires content.

`packages/ui/src/components/SessionChatBox.tsx` owns presentation. The
`onSend(message?: string)` argument is an explicit prompt override; ordinary
Send wraps the callback so an event cannot accidentally become prompt text.
The shared container passes the override through its existing prompt builder,
send hook, recovery and successful-send cleanup rather than mutating the draft
and immediately submitting a stale React closure. Draft restoration gates both
visible actions and explicit prompt submission.

Issue identity comes from `useWorkspaceIssueComments(workspaceId)`, already
used and cached by the conversation list. Its `issue.simple_id` is the readable
ID; the UUID is not a substitute. What’s left is disabled until this identity
is available. The query key includes host and workspace, keeping switches scoped.

The shared ChatBoxBase footer wraps controls and caps its action group's width
so multiple actions can fit narrow composers. Rendered tests live in
`packages/remote-web/src/test/SessionChatBox.test.tsx`. This repository uses
Vitest 2: assert call count and arguments separately, rather than using the newer
`toHaveBeenCalledExactlyOnceWith` matcher.

## Contributed by

- vk/bab2-button-prompts
