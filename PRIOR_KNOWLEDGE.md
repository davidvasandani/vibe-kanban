# Prior Knowledge: Settings-Owned MCPs in Worker Sessions

Relevant project knowledge is not empty.

## Distilled guidance

- `cluster-mcp-runtime-connectivity.md`: persistence, runtime adoption, and
  worker connectivity are separate boundaries. Authenticated definitions must
  be dispatched from settings; deployment cannot reconstruct them. The proven
  Codex pattern uses a bounded authenticated snapshot, an execution-scoped home,
  shared runtime/auth assets, no global worker mutation, and cleanup at job end.
- `shared-mcp-configuration.md`: native executor files are the definition source
  of truth. Deployment supplies immutable commands and prerequisites but must
  not seed a competing registry. Identifiers remain operational keys and secrets
  must not enter diagnostics.
- `active-mcp-refresh.md`: live reload is an executor capability. Codex refresh
  may update its scoped config and queue the vendor reload; unsupported
  executors adopt settings through a fresh process rather than claiming a live
  refresh.
- `workspace-environment-inheritance.md`: secrets belong at the narrow child
  process boundary, with deterministic precedence and no mutation of the
  long-lived service environment. Do not log or persist resolved maps.

## Consequences for this task

1. Generalize the existing Codex dispatch snapshot instead of inventing an
   environment-variable registry for MCP headers.
2. Preserve per-execution isolation for every executor; never update worker
   global native configs.
3. Make a scoped home overlay retain existing authentication/runtime assets and
   replace only the MCP-bearing config path.
4. Keep Codex refresh semantics unchanged and treat a new process as the
   adoption boundary for other executors.
5. Remove the repository `.mcp.json` Vibe entry because it is a competing
   project-scoped definition, not a persistence mechanism for Settings values.
