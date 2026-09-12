# Contract: `GET /api/info`

The existing successful `UserSystemInfo` payload gains one additive field:

```json
{
  "data": {
    "version": "ac5bedd",
    "deployment_timestamp": "2026-08-09T14:21:00Z"
  }
}
```

`deployment_timestamp` is an optional RFC 3339 UTC timestamp captured once for
the immutable release build and also written to `release.json`. It remains
constant across process and browser restarts. Unstamped development builds
return `null`. All existing response fields and error behavior are unchanged.

The generated TypeScript `UserSystemInfo` declaration is updated from the Rust
definition using `pnpm run generate-types`.
