# Contract: user-visible lookup failure messages

Prefix: `1Password lookup for <NAME> failed: ` (org Env Vars), or
`1Password lookup failed: ` (no variable, as in the GitHub owner paths,
which add `GitHub token for <owner>: `).

| Cause | Remedy text |
|---|---|
| RateLimited | `1Password rate-limited the service account (its request quota is used up); retry after the quota resets — \`op service-account ratelimit\` shows when` |
| Unauthorized | `1Password rejected the service-account token; check that OP_SERVICE_ACCOUNT_TOKEN is current and not revoked` |
| NotFound | `the vault, item, or field in the reference was not found or is not shared with the service account` |
| Unknown | `check the reference, service-account token, and vault permissions` |

Invariants: the message never contains the reference, the token, the
resolved value, or any provider output. HTTP mapping is unchanged (400 via
`ApiError::BadRequest`).
