# Technical Spec: Slack shortcut — AI-summarize the thread into title & description

> Task `vk/0f53-slack-shortcut-a`. Full SpecKit artifacts live in
> `homelab/specs/vk/0f53-slack-shortcut-a/` (`spec.md`, `plan.md`,
> `research.md`, `data-model.md`, `contracts/`, `tasks.md`). This file is the
> repo-root technical summary. Builds on the merged shortcut (PR #94, spec
> `fec4-vk-slack-shortcu`).

## Problem

The merged "Create issue from message" Slack shortcut prefills the issue modal
*mechanically*: title = the message's first non-empty line, description = the
message text + a permalink. For a threaded discussion where the decision emerges
over many replies, that captures one message, not the conversation.

## Solution

Add **optional, opt-in AI summarization** (the "Rovo-style" summary). When an
org admin enables it and supplies an Anthropic API key, invoking the shortcut
reads the whole Slack thread and generates a concise issue title + description
from it, swapping them into the already-open modal a beat after it opens. The
feature is strictly additive: with no key it is absent and the shortcut behaves
exactly as before, and any AI-path failure degrades to the mechanical prefill.

### 1. Schema — one migration (`crates/remote/migrations/20260711000000_slack_ai_summarization.sql`)

Two columns on `organization_slack_configs`: nullable
`encrypted_anthropic_api_key TEXT` (AES-256-GCM, write-only, absence = feature
off) and `ai_summarization_enabled BOOLEAN NOT NULL DEFAULT FALSE`. Effective
gate = `enabled AND ai_summarization_enabled AND key IS NOT NULL`
(`SlackConfig::ai_summarization_active()`).

### 2. Anthropic client (`crates/remote/src/anthropic/`)

New additive module. `AnthropicClient::summarize_thread(&[SlackReplyMessage])`
posts one request to `https://api.anthropic.com/v1/messages` (headers
`x-api-key`, `anthropic-version: 2023-06-01`) with model `claude-haiku-4-5` and
`output_config.format` forcing `{title, description}`; parses the first `text`
content block as JSON. HTTP-status errors (Jira pattern), never Slack's `ok`
envelope. The API key never appears in any error string. `prompt.rs` (pure)
builds the transcript with FR-16 caps (≤100 messages, ≤12000 chars, keep root +
most-recent, truncation marker).

### 3. Slack client (`crates/remote/src/slack/client.rs`)

`conversations_replies(channel, thread_ts, limit)`; `views_open` now returns the
created `view.id`; new `views_update(view_id, view)`.

### 4. Wiring (`crates/remote/src/routes/slack.rs::open_shortcut_modal`)

After the mechanical `views.open` (with a "✨ Summarizing thread…" hint when AI
is active), and only if `ai_summarization_active()`: fetch the thread
(`thread_ts` or `ts` for a standalone message), decrypt the key, summarize, then
`views.update` with the AI title/description (permalink still appended). On any
failure: `views.update` back to the plain mechanical modal and `warn`-log the
error class only (never the transcript or key). Runs inside the already-spawned,
signature-verified post-ack task, so it never risks Slack's 3s deadline.
`view_submission` is unchanged.

### 5. Admin API + settings UI

`SlackConfigResponse` gains `ai_summarization_enabled` + `has_anthropic_api_key`
(never the key). `UpsertSlackConfigRequest` gains `ai_summarization_enabled` +
`anthropic_api_key` (`None`/empty keeps stored). Key encrypted on save, not
validated (degrades at first use). `SlackSettingsSection.tsx` adds the toggle, a
masked key field, a privacy disclosure naming Anthropic + its ~30-day retention,
and four `*:history` scopes to the app manifest with a re-install note.

## Validation

`pnpm run check`, `pnpm run lint`, `cargo test --workspace`, `pnpm run format`.
Unit tests cover the transcript caps, structured-output parsing, the effective
gate, and `conversations.replies` parsing. Independent Codex review before merge.

## Constitution

Honors the new outbound-AI-egress principle (v0.8.0): off by default,
admin-controlled write-only encrypted key, graceful degrade, UI disclosure. The
inbound-interaction rule is preserved (AI runs post-verification, post-ack).
