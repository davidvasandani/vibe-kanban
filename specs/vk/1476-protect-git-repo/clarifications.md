# `/speckit.clarify`

Resolved 2026-08-10 from the existing branch-defaulting contract:

- Explicit caller initial branch wins when present.
- Repository-configured default is next.
- Built-in order is `origin/main`, `origin/master`, current, first.
- Remote prefixes are preserved verbatim.

No open questions remain.
