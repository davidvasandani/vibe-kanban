# Slack shortcut: optional AI thread summarization (ack-then-enrich + LLM egress)

How the "Create issue from message" Slack shortcut optionally drafts the issue
title/description from the whole thread with an LLM
(`crates/remote/src/anthropic/`, wired in `routes/slack.rs::open_shortcut_modal`).
The reusable shape is "user-initiated platform interaction + optional outbound
LLM enrichment", not Slack-specific.

## Ack-fast, enrich-later (never on the critical path)

Slack's interactivity deadline is 3s and the shared `http_client` timeout is
far longer, so **all** slow work is deferred: `handle_message_action` decrypts
the bot token, returns HTTP 200 immediately, and `tokio::spawn`s
`open_shortcut_modal`. The AI path lives entirely inside that spawned task,
*after* `views.open` has already shown the mechanical prefill. Consequence: a
slow/failed/absent LLM can never delay the ack or break the modal — the worst
case is the modal simply keeps the mechanical prefill. Do not "optimize" by
generating the summary before opening the modal; that would reintroduce the 3s
risk the base shortcut was designed around.

## The `views.update` swap and its mid-edit race

The modal opens (`views.open`, which we changed to **return the created
`view.id`**), then a single `views.update` swaps in the AI title/description.
`views.update` replaces the *whole* view, so a user mid-edit in the ~1–3s window
gets clobbered. v1 mitigation (not elimination): the initial modal carries a
`context` block "✨ Summarizing thread…" **only when the AI path will run**, so
the gate must be evaluated *before* building the initial modal. On AI failure we
issue one `views.update` back to the hint-less mechanical modal so no stale
"Summarizing…" notice lingers. The race-free alternative — a per-invocation
block_actions opt-in checkbox (Jira/Rovo style) — needs a new interaction
dispatch branch and is the documented future upgrade, not v1.

## Outbound LLM call: raw reqwest, structured outputs, HTTP-status errors

- No SDK. One `POST https://api.anthropic.com/v1/messages` over the shared
  `state.http_client`, headers `x-api-key` + `anthropic-version: 2023-06-01`,
  model `claude-haiku-4-5`. The base URL is a **code constant** — never
  caller-supplied — so there is no credential-exfiltration primitive (contrast
  the Jira connector's admin-supplied URL; see `external-connector-sync.md`).
- Anthropic returns real HTTP 4xx/5xx with a `{"error":{"type","message"}}`
  body, so the client follows the **Jira `check_status` pattern**, NOT Slack's
  in-band `{ok:false}` envelope. Copying the Slack client shape here would miss
  every error.
- Force `{title, description}` with **`output_config.format`** (json_schema);
  the first `content[]` block of `type:"text"` is then valid JSON to
  `serde_json::from_str`. Gotcha: structured outputs rejects string-length
  constraints (`maxLength`), so lengths are enforced by (a) the prompt and
  (b) post-truncation reusing `slack::prefill` (title via `title_from_message`,
  description via `description_from_message`, which also re-appends the
  permalink). Treat `stop_reason:"refusal"` as a summarization failure.

## Degrade-to-deterministic + secret hygiene (constitution v0.8.0)

Every AI-path failure — no/invalid key, thread-fetch `missing_scope`, provider
error/timeout, refusal, malformed JSON — returns `None` and the caller keeps the
mechanical prefill. Issue creation never depends on the AI (`view_submission`
uses whatever is in the modal at submit). The provider key is a **write-only
AES-256-GCM credential** on `organization_slack_configs`
(`state.jwt.encrypt_string`, COALESCE keep-on-`None`, `has_anthropic_api_key`
read indicator — same rules as the bot token). Two logging traps to avoid:
the `From<reqwest::Error>` must carry only `err.to_string()` (never the header),
and the failure `warn!` logs the error *class* only — never the thread
transcript (sensitive user content) or the key.

## Gates: effective-off must be all-three

`SlackConfig::ai_summarization_active()` = `enabled && ai_summarization_enabled
&& key present`. Any one false ⇒ no thread fetch, no LLM call, no hint — byte-
identical to the pre-AI shortcut. The migration column is **nullable**
(absence = off) and the toggle **defaults false**; on upsert the flag defaults
to the *stored* value when the request omits it (the column is set directly, not
COALESCE'd, so a partial save must not silently flip it off).

## Slack thread fetch

`conversations.replies(channel, thread_ts, limit)` — messages come oldest-first
(root at index 0), which the transcript cap relies on (keep root + most-recent).
`thread_ts = message.thread_ts.unwrap_or(message.ts)` so a standalone message
summarizes itself. Reading history needs a per-channel-type scope
(`channels:history` / `groups:history` / `im:history` / `mpim:history`); a
manifest scope change is **not** retroactive, so the settings UI must tell the
admin to re-install the app to grant them.

## Contributed by

- `vk/0f53-slack-shortcut-a`
