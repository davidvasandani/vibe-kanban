# Prior Knowledge: Remote MCP Configuration Synchronization (VAS-356)

The project knowledge base is populated and was searched read-only before
planning.

## Relevant pages

| Page | Distilled guidance |
| --- | --- |
| `cluster-mcp-runtime-connectivity.md` | Coordinator persistence, live-agent adoption, and worker reachability are distinct. Verify the exact executable and environment on the execution node. |
| `shared-mcp-configuration.md` | Native executor files are the source of truth. Preserve protocol identifiers, use existing adapters, and never substitute catalog presence for materialized configuration. Environment values are plaintext secrets and must not be logged. |
| `clustered-workspace-execution.md` | The worker owns assigned processes and their runtime prerequisites; coordinator-to-worker dispatch is authenticated, signed, idempotent, and the correct authority boundary for execution inputs. Runtime secret files must not enter the Nix store. |
| `workspace-environment-inheritance.md` | Keep credentials scoped to the child that needs them, apply execution-owned values at the spawn boundary, and avoid long-lived shared environment mutation. |
| `mcp-connectivity-testing.md` | Bound and redact diagnostics because MCP failures may contain credentials. Test the same definition the real client consumes. |

## Constraints carried forward

1. The coordinator's selected executor native MCP section is authoritative.
2. Synchronization belongs in the authenticated execution dispatch, not an
   unauthenticated side API, NFS secret file, Nix literal, or duplicated
   1Password item.
3. The worker must use the existing executor adapter and atomic native-config
   writer, preserving unrelated settings.
4. Snapshot application is execution setup, so failure must prevent the agent
   from starting with stale or silently incomplete MCP configuration.
5. Payloads require a conservative bound and diagnostics must mention only
   profile/config metadata, never headers or environment values.
6. Optional protocol fields preserve rolling compatibility with older workers.
