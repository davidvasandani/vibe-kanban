# Contract Change: `GET /api/info`

The existing successful `UserSystemInfo` response gains one backward-compatible optional field:

```json
{
  "version": "abc1234",
  "deployment_timestamp": "2026-08-09T14:22:31Z"
}
```

The response contains many existing fields omitted above.

## Semantics

- `deployment_timestamp` is the UTC build/publish time of the immutable running release.
- It is optional and serializes as `null` when the binary was not built by a timestamp-aware release process.
- Clients must tolerate absence, `null`, or an invalid timestamp and retain valid revision display without an age.
- `version: "dev"` remains the unstamped-build sentinel and is never treated as a source-control link.
