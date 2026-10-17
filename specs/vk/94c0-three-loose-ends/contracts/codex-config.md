# Contract: Fail-loud Codex configuration

- The executable command is the configured/pinned Codex base command followed by
  `app-server --strict-config` before user-supplied compatible parameters.
- Every execution uses strict mode, including deployment-managed base commands.
- Thread config never contains `include_apply_patch_tool`.
- Unknown settings in native or request-provided Codex config fail visibly at
  app-server startup/thread configuration rather than being ignored.
- The existing verified `features.unified_exec=false` control remains present.
