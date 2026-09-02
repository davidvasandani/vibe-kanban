# Contract: Background-helper rejection envelope

For each `StartBackgroundHelperError` variant, the response has:

- `success: false`;
- the unchanged typed variant in `error_data`;
- a nonempty `message` naming the rejected rule;
- a corrective action appropriate to the variant.

The MCP caller must never receive the fallback `Unknown error` for these
validation rejections.
