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

## The animated "Summarizing…" loading modal (Rovo-style)

When the AI path is active the shortcut opens a **dedicated loading modal**
(`build_summarizing_modal`) rather than the editable form — matching Jira Cloud
for Slack's Rovo card. `views.open` returns the created `view.id`; the shortcut
then **re-renders the modal each frame via `views.update`** to animate a
skeleton shimmer, and finally replaces it with the editable form (AI-filled on
success, mechanical prefill on failure/timeout). Key points:

- **Block Kit has no spinner or CSS**, so animation is *timed `views.update`
  frames*, not a hosted GIF (self-contained — no asset/route to serve). Each
  frame is a section heading + three inline-code skeleton bars with a bright
  band swept across them (`shimmer_bar(frame, row, width)` — the band moves with
  `frame` and rows are offset so the sweep reads diagonal). Inline-code wrapping
  keeps the cells monospace-aligned and gives the card a skeleton background.
- **The loading modal has no `input` blocks and no `submit` button** (Cancel
  only). That has a bonus: because the title/description inputs first exist only
  when we `views.update` to the form, their `initial_value` renders fresh — the
  input-state-preservation gotcha (below) does not bite, so the final form
  doesn't need the distinct-id trick.
- **Concurrency**: `animate_until_summarized` runs the summarize future and the
  frame ticker under one `tokio::select! { biased; … }` loop — it returns the
  instant the summary is ready, else after a hard `SUMMARIZE_TIMEOUT` (12s).
  The wait is bounded and always resolves to an editable form (the user can
  Cancel anytime), so the AI stays a convenience, never a block. Per-frame
  `views.update` errors are ignored (the modal may already be closed).
- `views.update` is well within Slack's rate limit (~18 frames max at 650ms).

### Input-state-preservation gotcha (still relevant for a non-loading swap)

`views.update` **preserves the current input value** for any block whose
`block_id`+`action_id` are unchanged and *ignores the new `initial_value`*. So
if you ever swap AI text into an *already-rendered* form (not the loading-modal
flow), the AI values won't show unless you give the inputs **distinct ids**
(`title_ai`/`description_ai`) and have `view_submission` accept either set — the
`ai_variant` path in `build_create_issue_modal`. The loading-modal flow sidesteps
this by having no inputs until the final render.

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
- `vk/0f53-slack-ai-animated-loading` (animated "Summarizing…" loading modal)
