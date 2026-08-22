# MCP Contract: `list_all_messages`

## Input

- `session_id?: UUID` — resolves the latest coding-agent execution.
- `execution_id?: UUID` — directly selects an execution and takes precedence
  when both identifiers are present.
- `roles?: string` — optional comma-separated role filter.

At least one target is required. The selected execution's owning session must
be visible in the MCP server's workspace scope.

## Output

The existing messages response object:

- `session_id`, `execution_id`, `status`, `exit_code`
- `final_message`
- chronological `messages[]` with `id`, `role`, `text`, `created_at`, and
  `execution_id`
- `has_more: false`

Errors use the existing MCP structured tool-error behavior.
