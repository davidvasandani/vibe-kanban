# SPEC — Auto Error Remediation

Task: `vk/7e4f-auto-error-remed`

## Problem

When a coding-agent turn fails, the chat shows an error and nothing else
happens. The operator has to notice it, write an issue, choose the pipelines,
choose repositories and an agent, and start a workspace. The request is to do
all of that automatically. The workspace should then run unattended, testing
the fix and merging it to `main`.

## Goals

1. When a chat error occurs, create an issue in a configured VK project. A chat
   error here means a coding-agent execution process that ends `Failed`.
2. Attach the **WikiLLM + SpecKit** pipelines to that issue with their default
   stages. Together those defaults already cover spec → recall → plan → SpecKit
   (constitution … implement) → Codex review → enrich knowledge → open **and
   merge** the PR, so no stage needs hand-picking for an unattended run to
   merge.
3. Start a workspace linked to that issue on the configured repositories
   (homelab + vibe-kanban for this deployment), with the configured executor
   profile and model (Claude Code, `claude-opus-5-5`, variant `PROALIGN`).
4. Make it safe to leave on: off by default, never remediate a remediation, no
   duplicates for the same source, and a global rate cap.

## Non-goals

- Triggering on turns that *complete* but contain an `error_message` entry.
  Finalization for a successful turn runs against the cleanup-script process,
  not the agent turn. Failed turns cover the chat-error case the operator sees.
  This is a possible follow-up.
- Persisting guard state across restarts. The guards are in-memory; see Risks.
- A new "profile" concept. The existing executor **variant** is VK's profile
  mechanism: `CmdOverrides` can carry env such as a separate
  `CLAUDE_CONFIG_DIR`.

## Interpretation of ambiguous inputs (no answer received; recorded as assumptions)

| Request wording | Interpretation | Why |
|---|---|---|
| "the vk project" | Remote project **Vibe Kanban** (`e4d12693-…`, Vasandani org). It is configured by id in settings; when unset, the failing workspace's own linked project is used. | It is the only project with that name. Configuration keeps it off the hard-coded path. |
| "proalign profile" | Executor profile variant `PROALIGN` on `CLAUDE_CODE`. If that variant is not defined in the live `profiles.json`, fall back to the executor's default variant, log a warning, and note it in the issue. | The code has no "proalign" executor variant. The ProAlign *organization* has no VK project. The variant is the profile selector in the UI. |
| "opus 5.5" | `model_id = "claude-opus-5-5"` | This is the catalog id in `crates/executors/src/executors/claude.rs`. |
| "homelab and VK repos" | Configured `repo_ids`. When empty, use the failing workspace's repos with their target branches. | Repo UUIDs are deployment data, not code. |
| "error in the chat" | A `CodingAgent` execution process finalized as `Failed`. `Killed`, `Interrupted` and `Indeterminate` are excluded. | These are exactly the turns that render as a failed chat turn. User stops and restarts are not errors. |

## Design

### Configuration (`Config` v8, additive, `#[serde(default)]`)

```rust
pub struct AutoErrorRemediationConfig {
    pub enabled: bool,                     // default false
    pub project_id: Option<Uuid>,          // None → failing workspace's remote project
    pub repo_ids: Vec<Uuid>,               // empty → failing workspace's repos
    pub executor: BaseCodingAgent,         // default CLAUDE_CODE
    pub variant: Option<String>,           // default Some("PROALIGN")
    pub model_id: Option<String>,          // default Some("claude-opus-5-5")
    pub pipeline_ids: Vec<String>,         // default ["wikillm", "speckit"]
    pub merge_stage_ids: Vec<String>,      // default ["pr-and-merge", "merge"]
    pub max_per_hour: u32,                 // default 3, global cap
}
```

The field on `Config` is `auto_error_remediation`. Old config files
deserialize with defaults. Types are regenerated into `shared/types.ts`.

### Trigger (services)

`ContainerService::finalize_task` already runs once for every finalized
execution across all local and worker exit paths. It sends notifications, and
it now also emits an `ErrorRemediationEvent` when:

- `run_reason == CodingAgent` and `status == Failed`, and
- the config is enabled (read at emit time).

The event carries `workspace_id`, `session_id` and `execution_process_id`. It
travels on a `tokio::sync::broadcast` channel exposed through a new trait
method `error_remediation_sender()`. The trait default is `None`, and the local
container owns the sender. If no receiver exists the send is a no-op.
`finalize_task` must stay fast and infallible, so there is no network I/O on
this path.

### Launcher (server)

A background task subscribes at startup (`startup.rs`). For each event it
does the following:

1. **Guard** (`ErrorRemediationGuard`, pure and unit-tested):
   - Skip if the source workspace is itself a remediation workspace: its name
     starts with `AUTO_REMEDIATION_NAME_PREFIX` (`"Auto-fix: "`), or its id is
     in the in-memory set of launched workspaces.
   - Skip if that source workspace was already remediated in the last 24 h
     (dedupe).
   - Skip if `max_per_hour` launches already happened in the trailing hour.
2. **Collect context:** the last `error_message` entries from
   `container.normalized_entries(exec_id)`, capped in count and length. Also
   the workspace name, branch, executor and exit code.
3. **Create the issue** through `RemoteClient`:
   - Title: `Auto-fix: <workspace name> agent run failed`.
   - Status: the project's first visible status, the same rule as MCP
     `default_status_id`.
   - Priority: high.
   - Description: the error context, an HTML marker
     `<!-- vk:auto-remediation source=<ws> exec=<exec> -->`, and the pipeline
     block composed from the configured pipelines' default stages.
4. **Start the workspace** by calling the existing
   `create_and_start_workspace` handler directly (not over HTTP). It uses the
   issue as `linked_issue`, the prompt `title + description` (as MCP
   `start_workspace` builds it), the name `Auto-fix: …`, and an
   `ExecutorConfig` built from the config.
5. **Link** the workspace to the issue through the existing `link_workspace`
   handler, as the UI and MCP do.
6. Record the launch in the guard. Failures are logged at `warn` and never
   retried, to avoid loops.

### Shared pipeline-block composer

`compose_pipeline_block`, `canonical_stage_order`, `compose_executor_line` and
`append_pipeline_block` move out of `crates/mcp` (they were `pub(crate)`) into
`api-types::pipeline_block`, which operates on a minimal
`BlockPipeline { name, stages: Vec<BlockStage { id, prompt_fragment } > }`. The
MCP and the server launcher both map into it. The existing tests move with the
code and keep byte-identical output, so the format still mirrors
`taskPipeline.ts`.

### Frontend

The General settings gain an **Auto error remediation** card, following the
`resume_interrupted_on_startup` precedent:

- An enable toggle, with a warning that it spawns agents unattended and they
  can merge to main.
- A project id field.
- Repositories as a multi-select of repos.
- An executor profile picker (the existing `ExecutorProfileSelector`).
- A model id field.
- A max-per-hour field.

i18n keys are added for all locales, with English copy in all of them as the
existing precedent does.

## Acceptance criteria

1. With `enabled=false` (the default), a failed agent turn creates no issue and
   no workspace. The event is still emitted, because `ContainerService` has no
   config accessor, and the consumer drops it after reading the flag.
2. With `enabled=true`, a failed `CodingAgent` turn creates exactly one issue in
   the configured project. The issue's description contains the error context,
   the remediation marker, and a `## Pipeline: WikiLLM + SpecKit` block. The
   block comes from the shared composer, using each pipeline's default stages
   plus a merge stage (`merge_stage_ids`) when no default merges. With this
   deployment's pipelines, where `pr-and-merge` is already a default, it is
   byte-identical to MCP `create_issue(pipeline_ids=[wikillm,speckit])`.
3. A workspace named `Auto-fix: …` starts, linked to the issue, on the
   configured repos, with executor `CLAUDE_CODE`, variant `PROALIGN` (or the
   default when that is missing) and model `claude-opus-5-5`.
4. A failure inside an `Auto-fix:` workspace never triggers another
   remediation.
5. A second failure in the same source workspace within 24 h, and any launch
   beyond `max_per_hour`, is skipped with an `info` log.
6. `Killed`, `Interrupted` and `Indeterminate` runs, and non-agent processes,
   never trigger.
7. Unit tests cover the guard, the trigger predicate, issue-body composition
   and the moved composer. `cargo test`, `pnpm run check`, `pnpm run lint` and
   `pnpm run generate-types:check` pass.

## Risks

- **Runaway spend or merges.** Mitigated by opt-in, the loop guard, dedupe and
  the hourly cap. Because the state is in-memory, a restart resets the counters
  but not the name-prefix loop guard. The worst case is `max_per_hour` launches
  per restart.
- **Missing `PROALIGN` variant.** This degrades to the default variant rather
  than failing the remediation, and the issue says so.
- **Remote not configured or logged out.** Issue creation fails and is logged;
  there is no local-only fallback.

## Deployment note

The feature ships disabled. Enabling it for this deployment needs these
settings:

- project `e4d12693-c789-4238-a1f3-4ccb998c8279`
- repos homelab `b2a286a2-1831-47e0-b4b8-b55239b49a2a` and vibe-kanban
  `cdce12c2-a050-49b1-86c3-3b28ace8ada8`
- a `CLAUDE_CODE` → `PROALIGN` variant defined in profiles

Nothing needs to change in `homelab/modules/vibe-kanban-rebuild.nix`, because
the settings live in the VK config.

---

# Follow-up: reuse active issues with similar errors

Requested after the first merge (#331): "AER should look for active issues
with similar errors before creating a new one."

## Problem

The source-workspace dedupe only stops *the same workspace* from filing twice.
When one root cause breaks several workspaces, for example an expired
credential or a broken upstream API, each of them files its own issue and
starts its own unattended workspace. That produces parallel agents racing to
fix and merge the same thing.

## Design

1. **Error fingerprint.** `error_fingerprint(messages)` normalizes the
   captured error messages. It lowercases them, replaces UUIDs, long hex runs
   and digit runs with placeholders, and collapses whitespace. It then hashes
   the result with FNV-1a 64 into 16 hex characters. Messages that differ only
   in ids, timestamps, ports or line numbers get the same fingerprint. When no
   error message was captured there is no fingerprint, and the new behavior is
   skipped. Two empty contexts carry no evidence of the same cause.
2. **Marker.** The issue marker gains the fingerprint:
   `<!-- vk:auto-remediation source=… exec=… fingerprint=<fp> -->`. It is
   omitted when there is no fingerprint.
3. **Similarity.** Two error texts are *similar* when either of these holds:
   - Their fingerprints are equal.
   - The Jaccard similarity of their normalized token sets is at least 0.8.

   The token check catches near-duplicates, such as a changed file name, and
   issues filed before fingerprints existed. The existing issue's error text
   is parsed back out of the `### Error messages` fence.
4. **Lookup, before creating.** Search the target project with
   `search_issues`, using `search = "vk:auto-remediation"` and `status_ids`
   set to the project's **active** statuses. Active excludes `done`,
   `cancelled` and `canceled`, case-insensitively, which is the remote's
   existing convention. Only issues that carry the marker count.
5. **On a match.** Post a comment on the existing issue (`POST
   /v1/issue_comments`). The comment records the new source workspace,
   execution, exit code and a bounded error excerpt. No issue is created and
   no workspace is started. On no match, proceed as before.
6. **Fail closed.** If the lookup itself fails (remote error), skip the
   launch and log it. That is consistent with XLI: a guard failure never
   spawns. A comment failure is logged; the occurrence has already been
   routed to the existing issue, so it is not a reason to launch.
7. The in-memory guard reservation (dedupe and hourly cap) still happens
   first. A matched occurrence therefore consumes a slot, which also bounds
   comment volume.

## Acceptance criteria

- A failure whose errors match an active AER issue in the target project adds
  a comment to that issue. It creates no issue and starts no workspace.
- A match against an issue in Done or Cancelled is ignored, and a new issue is
  filed.
- Errors that differ only in UUIDs, numbers or hex ids match. Different
  errors do not match. A failure with no captured error never matches.
- A failed lookup launches nothing.
- The new logic has unit tests: fingerprint normalization, similarity
  threshold, error-text round-trip from the issue body, marker parsing and
  active-status selection.
