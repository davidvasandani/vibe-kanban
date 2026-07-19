# Implementation Plan: VK MCP Auto Debug

Task: `9453-vk-mcp-auto-debu`

This initial plan incorporates `SPEC.md` and `PRIOR_KNOWLEDGE.md`. The later
SpecKit plan stage may refine file-level details after constitution and
clarification checks.

1. Trace the MCP assignment-result render path, the settings dialog's provider
   boundaries, local project selection, issue insertion/synchronization, issue
   navigation, clipboard helpers, notifications, and existing test harnesses.
2. Define small pure helpers for selecting the exact diagnostic fallback and
   building a safe, deterministic issue title/Markdown description containing
   server, executor, error, and fix/verification instructions.
3. Add failed-result UI that renders multiline diagnostics without truncation
   and exposes accessible Copy and Debug buttons while leaving `auth_required`,
   `unsupported`, and `ok` presentation unchanged.
4. Wire Copy to the clipboard with explicit pending/success/failure feedback and
   no mutation of test state.
5. Wire Debug to the existing local VK project/issue mutation path, using only
   explicit current project context, guarding duplicate clicks, reporting
   failures inline, and surfacing/navigating to the created issue when supported
   by the surrounding route.
6. Add English settings translations and preserve existing locale fallback
   behavior; update other locale resources only if repository convention or
   validation requires key parity.
7. Add focused tests for full diagnostic rendering, exact copy content, safe
   issue description construction, correct issue payload/project, missing
   project context, mutation errors, and duplicate-click prevention.
8. Run targeted tests, frontend type checking/linting, and repository formatting;
   repair any regressions.
9. Complete the mandated independent Codex diff review, address confirmed
   findings, rerun verification, and then record reusable architecture knowledge
   in the project knowledge base and its index.
