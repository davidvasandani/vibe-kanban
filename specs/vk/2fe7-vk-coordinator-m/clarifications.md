# Clarifications: Coordinator Workspace Placement

## Resolved questions

### Should the coordinator choice be conditional on cluster mode?

No. The create form will show **Coordinator** wherever it already shows the **Run on** selector. In a clustered deployment it is the explicit coordinator-local placement choice. In a standalone deployment all execution is local already, so selecting it is harmless and produces the same local outcome.

This avoids adding a cluster-capability discovery request solely to hide a semantically valid option. The server remains authoritative: it interprets coordinator intent specially only when clustering is enabled and otherwise preserves the existing standalone-local path.

## Remaining open questions

None.
