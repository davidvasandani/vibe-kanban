# `/speckit.constitution`: Stable Streaming Conversation Viewport

The current project constitution is version 0.31.0. It already contains
Principle XXXVI, “Dynamic viewports have one scroll authority,” which directly
governs this task: follow-tail, preserve-anchor, and named navigation are
mutually exclusive policies, and layout-boundary transitions must be monotonic
during a live update.

No constitution edit is required. The task also follows contract-focused tests,
small reversible changes, shared `web-core` ownership, reuse of existing
virtualization machinery, and worktree-safe verification.
