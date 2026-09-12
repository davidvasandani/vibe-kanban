# Clarifications: Server Affinity Sidebar Polish (`61a3`)

## Resolved questions

### Should header context appear only while collapsed?

**Decision:** Keep the server context in the header in both expanded and
collapsed states. The task's required guarantee is that collapsing does not
remove the server name. Keeping one stable header avoids a layout shift and
matches the existing section-header extension contract; the expanded body may
still show the explicit “Current server” row because it explains the control
below it.

### What counts as “server name” without a resolved hostname?

**Decision:** Use the existing summary precedence: assigned worker hostname,
then requested worker hostname, then the translated placement kind. Do not
invent a hostname or expose an opaque worker UUID as header context.

### How should spacing be judged?

**Decision:** Use a compact aligned label/value layout based on existing sidebar
spacing tokens. The selector fills the remaining value column up to its existing
reasonable maximum, rather than being pushed to the far edge by
`justify-between`. Pixel-perfect screenshot matching is not required; absence
of overflow and clear label/value association are the contract.

## Remaining open questions

None.
