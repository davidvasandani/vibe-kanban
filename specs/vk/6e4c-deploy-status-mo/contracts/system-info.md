# Contract: `GET /api/info`

The existing successful `UserSystemInfo` payload gains one additive field:

```json
{
  "data": {
    "version": "ac5bedd",
    "started_at": "2026-08-09T14:21:00Z"
  }
}
```

`started_at` is an RFC 3339 UTC timestamp representing when the current server
process initialized its HTTP routes. It remains constant for the process
lifetime. All existing response fields and error behavior are unchanged.

The generated TypeScript `UserSystemInfo` declaration is updated from the Rust
definition using `pnpm run generate-types`.
