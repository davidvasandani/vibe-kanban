# Worker Contract: Codex Session Transfer

All routes are inside existing signature middleware. Mutations carry
`RequestAuthority` with `correlation_id = operation_id`; path/body IDs and
`authority.worker_node_id` must agree with the contacted worker.

## Source manifest

`POST /v1/session-transfers/{operation_id}/manifest`

Request binds workspace, source execution, source/target workers, leaf thread,
and limits. Response returns ancestor-first entries with safe relative path,
canonical thread/parent/relation, size, SHA-256, and canonical manifest digest.

## Source chunk

`POST /v1/session-transfers/{operation_id}/source-chunk`

Request repeats manifest digest, thread/checksum/size, offset, and bounded
length. Response returns offset, base64 bytes, EOF, and chunk SHA-256. Source
revalidates the open regular file and whole-entry identity; mismatch aborts.

## Target stage chunk

`POST /v1/session-transfers/{operation_id}/target-chunk`

Request repeats context/manifest/entry, offset, bytes, and chunk digest. Target
accepts only the next offset for the operation-scoped partial. Response returns
accepted length and next offset. Identical replay is idempotent; divergent
replay is conflict.

## Target finalize entry

`POST /v1/session-transfers/{operation_id}/finalize-entry`

Target validates full size/checksum, metadata identity, safe destination,
ownership/permissions, and installs without overwrite. Existing identical
content returns `reused`; different content returns `target_conflict`.

## Target verify/status

`POST /v1/session-transfers/{operation_id}/verify`

Target reopens every manifest entry, confirms complete ordered identity and
digest, and returns per-entry safe evidence plus verified manifest digest/time.
No contents are returned.

## Abort partials

`POST /v1/session-transfers/{operation_id}/abort`

Removes only operation-scoped partials after containment/type checks. Verified
artifacts are never removed by abort.

Errors use an allow-listed code, phase, remediation, and bounded safe facts.
They never contain rollout lines or raw executor errors.
