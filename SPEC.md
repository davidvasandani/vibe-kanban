# Technical Specification: Claude Opus 5 Model Support

## Objective

Expose the newly released Claude Opus 5 model in Vibe Kanban's hard-coded
executor model selectors so users can choose it when the backing agent supports
the model.

Anthropic's current model documentation identifies the API model ID as
`claude-opus-5`. Provider-specific model names must follow each executor's
existing naming convention.

## Scope

- Add Claude Opus 5 to applicable hard-coded executor model catalogs.
- Preserve provider-specific identifiers and user-facing naming conventions.
- Add any executor-specific reasoning/variant resolution needed for selecting
  the new model.
- Refresh generated schemas when source schema metadata changes.
- Add or update focused tests for model-name resolution and catalog discovery.

## Requirements

1. Claude Code must offer an explicit `"claude-opus-5"` entry (display label
   `"Opus 5"`) alongside its existing aliases, following the `"claude-sonnet-5"`
   precedent. Existing generic aliases (`"opus"`, `"opus[1m]"`) remain unchanged.
2. Executors whose published integrations support Opus 5 and whose model lists
   are maintained in Vibe Kanban must expose the provider-correct identifier.
3. Cursor-specific reasoning selection must resolve Opus 5 to the correct
   standard and thinking identifiers if Cursor supports the model.
4. Existing Opus versions must remain selectable.
5. Generated artifacts must be produced through the repository's documented
   generation commands, not edited by hand.

## Verification

- Run focused Rust tests for every changed executor.
- Run schema/type generation checks relevant to changed metadata.
- Run repository formatting.
- Run broader compilation or checks in proportion to the final change surface.
- Complete an independent Codex diff review and resolve all significant
  findings.

## Non-goals

- Changing default models.
- Removing or deprecating older Claude models.
- Bumping agent CLI package versions.
- Adding unsupported fast-mode variants without provider evidence.

## External Source

- Anthropic Claude Platform model overview:
  https://platform.claude.com/docs/en/about-claude/models/overview
