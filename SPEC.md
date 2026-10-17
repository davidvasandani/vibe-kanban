# VK Browser keyboard and clipboard

## Problem and scope
The streamed browser is an image with pointer and key forwarding. Focusing its container cannot summon a mobile software keyboard, and native clipboard actions currently affect the local page rather than the remote browser. Limit changes to the Vibe Kanban repository.

## Required behavior
- A controlling user tapping a remote text field can type with their device keyboard, including composition, deletion, and Enter.
- Provide explicit keyboard access if remote focus cannot be identified synchronously within the mobile user gesture.
- Users can paste plain text from their device into the focused remote field and copy remote selected text back to their device.
- Clipboard access is explicit; denied or unsupported access has a selectable/manual fallback and visible error feedback.
- Preserve control ownership, stale-generation rejection, desktop keyboard behavior, and session isolation. Read-only viewers cannot send input.
- Never persist clipboard contents or log typed text.

## Technical direction
Inspect the existing browser WebSocket/CDP protocol and extend its guarded input handling only where needed. Use a real local editable input for mobile keyboard events, rather than depending on keydown alone. Bridge plain text through existing input routing where possible; remote selection retrieval must respect session control. Clear ephemeral input/clipboard state when sessions or ownership change.

## Verification
Cover text/composition event handling, clipboard behavior, ownership rejection and session changes with focused tests. Run repository formatting, type checks, relevant Rust tests and lint. Independently review the diff, fix confirmed findings, document reusable behavior, and merge a PR after checks succeed.
