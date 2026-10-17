# Clarifications — vk/dc99-show-left-drawer

No questions were sent to the requester. The task is a short UI request with a
screen recording. Each answer below is the least surprising choice for that
request, and each one is cheap to change later because it is a constant or a
single branch.

## Q1. What widths decide "no space"?
**Answer:** The drawer and the right sidebar are each 300px (the current fixed
widths). The chat needs at least 480px, which keeps the composer toolbar on
one line, and an open right panel needs at least 400px. The fit rule is:

    workspace width ≥ 300 + (right sidebar shown ? 300 : 0)
                         + (chat shown ? 480 : 0)
                         + (right panel open ? 400 : 0)

The recording's window is about 1300px. Its workspace area (the window minus
the ~130px app rail) needs 1080px, so the drawer shows. With Changes open the
rule needs 1480px of workspace area: a 1920px screen fits, a 1440px laptop
fits only with the right sidebar closed. The old rule hid the drawer on
anything up to 2048px.
**Rejected:** a window breakpoint (`innerWidth`). It ignores the rail, the
docked Settings drawer and which panels are open (constitution LIV; wiki
settings-drawer.md).

## Q2. What happens to an old saved "hidden" preference?
**Answer:** Treat it as the user's choice. A false written by the old
auto-collapse looks the same as a deliberate hide. These recovery paths
already exist and stay: closing a right panel restores the drawer, and one
`V S` restores it. No migration and no new scratch field.
**Rejected:** forcing the preference to true once. That would undo real
choices and needs a server-side marker in the typed scratch.

## Q3. How long does "show anyway" last when there is no space?
**Answer:** It lasts until the fit result changes (the layout re-evaluates
as fitting, which ends the need for it) or the user hides the drawer. It is
transient and not persisted. Because the measured area is the whole
workspace root, showing the drawer does not change the measurement, so the
override cannot flicker.
