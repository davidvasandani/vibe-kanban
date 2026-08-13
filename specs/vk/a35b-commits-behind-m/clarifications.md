# `/speckit.clarify`: Commits Behind in the Git Header

**Resolved during:** 2026-08-13

## Decisions

1. **Only positive behind counts are listed.** Zero values add noise and weaken
   the useful meaning of an otherwise absent warning. Missing/loading values are
   also omitted because they are not evidence that a branch is current.
2. **Single-repository copy is `<count> behind`.** The surrounding Git section
   and sole repository make the subject unambiguous, while the word `behind`
   makes the direction clear without relying on color or icon interpretation.
3. **Multi-repository copy is `<repo> <count>` joined by a middle dot.** The
   repository label preserves the mapping requested by the user and the compact
   shape fits the existing header. Full title/accessible text spells out
   `<repo> is <count> commit(s) behind` for each entry.
4. **Workspace repository cardinality controls naming.** If a workspace has
   multiple configured repositories but only one is behind, that one value still
   includes its repository name; otherwise the user could not know which of the
   several repositories it describes.

## Remaining questions

None.
