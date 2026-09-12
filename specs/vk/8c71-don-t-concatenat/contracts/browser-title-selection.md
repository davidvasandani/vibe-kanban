# Browser Title Selection Contract

Given ordered candidates `candidates: (string | null | undefined)[]`:

1. Find the first string for which `candidate.trim().length > 0`.
2. If found, assign the candidate with surrounding whitespace removed as the
   complete `document.title`.
3. Otherwise, assign exactly `Vibe Kanban`.
4. Never join candidates or add prefixes, suffixes, ticket IDs, or separators.
5. Re-evaluate after a candidate value or order changes.

Examples:

| Candidates | Browser title |
| --- | --- |
| `['Issue title', 'Project']` | `Issue title` |
| `[undefined, 'Project']` | `Project` |
| `['   ', '  Project  ']` | `Project` |
| `[null, undefined, '']` | `Vibe Kanban` |
