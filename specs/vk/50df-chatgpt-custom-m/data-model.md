# Data model (Vibe Kanban SQLite)

Migration `crates/db/migrations/20261004000000_mcp_oauth.sql`. Timestamps
are stored as `TEXT` RFC 3339 UTC, matching the existing `datetime('now',
'subsec')` style used for comparisons. Hashes are SHA-256 `BLOB`s.

## mcp_oauth_clients
| column | type | notes |
|---|---|---|
| client_id | TEXT PK | random 128-bit, base64url |
| client_secret_hash | BLOB NULL | NULL for `none` |
| token_endpoint_auth_method | TEXT | `none` \| `client_secret_post` \| `client_secret_basic` |
| client_name | TEXT NULL | ≤ 200 chars |
| redirect_uris | TEXT | JSON array, 1–10 entries |
| created_at | TEXT | |

Pruned when older than 24 h and with no grant. Capped at 200 rows.

## mcp_oauth_authorizations
| column | type | notes |
|---|---|---|
| id | TEXT PK | uuid |
| client_id | TEXT FK → clients ON DELETE CASCADE | |
| redirect_uri | TEXT | exact registered value |
| code_challenge | TEXT | S256 |
| scope | TEXT | `mcp` |
| resource | TEXT | resource URL |
| state | TEXT NULL | echoed |
| consent_hash | BLOB | one-time consent token |
| code_hash | BLOB NULL UNIQUE | set on approve |
| status | TEXT | `pending` → `approved` → `exchanged`; or `denied` |
| grant_id | TEXT NULL | set on exchange (for replay revocation) |
| expires_at | TEXT | pending: +10 min; approved: +60 s |
| created_at | TEXT | |

## mcp_oauth_grants
| column | type | notes |
|---|---|---|
| id | TEXT PK | uuid |
| client_id | TEXT FK → clients ON DELETE CASCADE | |
| scope, resource | TEXT | |
| created_at | TEXT | |
| last_used_at | TEXT NULL | updated by verify (≤ 1/min) |
| revoked_at | TEXT NULL | |

## mcp_oauth_tokens
| column | type | notes |
|---|---|---|
| token_hash | BLOB PK | |
| grant_id | TEXT FK → grants ON DELETE CASCADE | |
| kind | TEXT | `access` \| `refresh` |
| expires_at | TEXT | access +1 h, refresh +30 d |
| revoked_at | TEXT NULL | rotation and revocation |
| created_at | TEXT | |

Indexes: `tokens(grant_id)`, `authorizations(expires_at)`,
`clients(created_at)`.

## State transitions

- **authorize GET** → insert `pending`.
- **authorize POST approve** → the consent hash matches, the row is
  `pending` and unexpired → `approved` with `code_hash` and a new expiry.
- **POST deny** → `denied`.
- **token(code)**, in one transaction: the row is `approved`, unexpired,
  and matches client, redirect and PKCE → `exchanged`, insert grant and
  tokens, set `grant_id`. If the row is already `exchanged`, revoke its
  grant and return `invalid_grant`.
- **token(refresh)**, in one transaction: the token is `refresh`,
  unrevoked, unexpired, its grant is unrevoked and the client matches →
  revoke it and insert a new pair. If the token is already revoked and its
  grant is live, revoke the grant (reuse detection) and return
  `invalid_grant`.
- **revoke grant** → set `grants.revoked_at`. Verify checks it.
