# SessionChatBox's stopped-run banner shares the footer's icon buttons

`SessionChatBox` (`packages/ui/src/components/SessionChatBox.tsx`) renders
an `interruptedNotice` banner when a session's latest coding-agent process
has `ExecutionProcessStatus.interrupted` (see
[[coordinator-restart-handoff]] for how a worker/coordinator restart
produces that status). The banner and the composer footer are not
independent: the footer's icon-only buttons (attach file, GitHub
PR-comment insert, any `toolbarActions` items) are defined once, in a
`renderIconButtons()` closure, and conditionally rendered in exactly one of
two places — `ChatBoxBase`'s `banner` slot while `interruptedNotice` is set,
or its `footerLeft` slot otherwise. They are never rendered in both.

**If you touch either the footer icon buttons or the interrupted banner,
check the other.** Adding a new icon-only toolbar action, for example,
means adding it inside `renderIconButtons()` — not directly inside
`footerLeft` — or it will silently fail to appear while a session is
stopped. Conversely, anything added directly to the banner JSX (not through
that closure) only shows up in the stopped case.

The footer's status-driven action button (`footerRight`, e.g. `Send`) is
untouched by this: it keeps rendering in every status including
interrupted, so typing a follow-up and sending it has always worked as an
alternative to clicking the banner's restart button. Only the icon buttons
moved; `footerRight` was deliberately left out of the relocation (see
`specs/vk/556e-start-stopped-se/plan.md`'s "Research / Alternatives
considered" — pulling `Send` out while stopped was a bigger, unrequested
behavior change).

## Contributed by

- vk/556e-start-stopped-se
