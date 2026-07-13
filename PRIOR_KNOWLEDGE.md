# Prior Knowledge — recalled for `vk/0f53-slack-shortcut-a`

Searched the project knowledge base (`wiki/` — 11 topic pages + INDEX) for
pages relevant to this task (adding optional AI summarization to the merged
Slack "Create issue from message" shortcut). One page is directly on-topic;
the rest are unrelated (kanban UI, sync, deployment, agent lifecycle).

## Relevant findings

**[wiki/external-connector-sync.md] — the credential pattern this task reuses
wholesale.** Written for the Jira connector but explicitly connector-agnostic:

- Connectors live in the **remote** stack (`crates/remote`), because the board
  users see is Postgres/Electric, not the local SQLite model. The Slack
  integration already lives there (`crates/remote/src/slack/`); the Anthropic
  key rides on the same `organization_slack_configs` row.
- **Credential storage rule (applied verbatim to the Anthropic key):** store as
  `state.jwt.encrypt_string(...)` ciphertext (AES-256-GCM); read APIs expose
  only a `has_*: bool` indicator, never the secret; `null`/empty on update means
  "keep" (COALESCE in the repo upsert). Credential-bearing routes gate on org
  admin (`assert_admin`).
- **The destination-pinning / exfiltration rule** ("a stored secret may only be
  sent to the stored destination") is why the Anthropic base URL is a **code
  constant** (`https://api.anthropic.com/v1/messages`), never caller-supplied —
  there is no exfiltration primitive here, unlike Jira's admin-supplied URL.

## What this task adds beyond prior knowledge (candidate for a new page)

Nothing in the KB covers **outbound AI/LLM egress** or the **Slack
interactivity ack-then-enrich** flow. New, reusable material this task
established (distilled in the knowledge-base stage into
`slack-shortcut-ai-summarization.md`):

- The Slack shortcut's ack-fast/enrich-later shape: ack ≤3s, `views.open` the
  mechanical modal, then in the *same* spawned task fetch the thread + call the
  LLM + `views.update`. Enrichment is strictly optional and post-ack.
- Outbound LLM call = raw reqwest over the shared `http_client`, Jira-style
  HTTP-status errors (not Slack's `ok` envelope), structured outputs via
  `output_config.format` (json_schema; `maxLength` unsupported, so lengths are
  enforced by prompt + post-truncation reusing `prefill.rs`).
- The universal degrade-to-deterministic rule (constitution v0.8.0): every AI
  failure falls back to the mechanical prefill; the key/thread text never hit
  logs or error strings.
- The `views.update` mid-edit race and its v1 mitigation (a "✨ Summarizing…"
  hint + single fast update; block_actions checkbox is the deferred upgrade).

## Notes

The base shortcut (`vk/fec4-vk-slack-shortcu`, PR #94) has a full SpecKit spec
under `homelab/specs/vk/fec4-vk-slack-shortcu/` but had no wiki page — this
task's knowledge-base entry is the first for the Slack integration.
