# Implementation Plan: Slack AI summarization (task `vk/0f53-slack-shortcut-a`)

Step-by-step build order. The authoritative dependency-ordered task list is
`homelab/specs/vk/0f53-slack-shortcut-a/tasks.md` (T001–T019); this is the
executable narrative. Rationale in `SPEC.md`; prior-art recall in
`PRIOR_KNOWLEDGE.md`.

## Step 1 — Schema + config storage (T001–T002)

Migration `crates/remote/migrations/20260711000000_slack_ai_summarization.sql`
adds nullable `encrypted_anthropic_api_key TEXT` and
`ai_summarization_enabled BOOLEAN NOT NULL DEFAULT FALSE`. In
`db/slack_configs.rs`: both columns into `SELECT_COLUMNS`; `UpsertSlackConfigArgs`
gains the two fields; the `upsert` gets `COALESCE($8, …)` for the key
(write-only keep-on-None) and sets the flag directly (`$9`).

## Step 2 — Anthropic client (T003–T006)

New `crates/remote/src/anthropic/`: `types.rs` (`IssueSummary`, response +
error bodies), `prompt.rs` (pure — system prompt + FR-16-capped transcript with
unit tests), `client.rs` (`AnthropicClient::summarize_thread`, HTTP-status
errors, structured-outputs request, first-text-block JSON parse; `AnthropicError`
whose `Transport`/`Api` variants never carry the key). Register
`pub mod anthropic;` in `lib.rs`.

## Step 3 — Slack client + types (T007–T008)

`slack/types.rs`: `SlackConfig` FromRow gains the two columns +
`ai_summarization_active()`; DTOs gain the AI fields; add `thread_ts` to
`SlackMessageRef`; add `SlackConversationsRepliesResponse`/`SlackReplyMessage`
and `SlackViewsOpenResponse`. `slack/client.rs`: `conversations_replies`,
`views_open` returns the view id, `views_update`.

## Step 4 — Modal hint (T009)

`slack/modal.rs`: `build_create_issue_modal` gains a `hint: Option<&str>` param;
when `Some`, a leading `context` block renders the "✨ Summarizing thread…"
notice. All existing callers/tests pass `None`.

## Step 5 — Wiring (T010)

`routes/slack.rs`: `handle_message_action` passes `ai_active` +
`encrypted_anthropic_api_key` into the spawned `open_shortcut_modal`. There, the
gate is computed **before** building the initial modal (so the hint is included
only when AI will run — resolves the analyze `[error]`); `views.open` captures
the view id; then `summarize_thread_for_modal` fetches the thread
(`thread_ts` else `ts`), decrypts, summarizes, and the caller `views.update`s the
AI result (permalink appended) or reverts to the plain modal on failure. Logs
are error-class only (never transcript/key — resolves the analyze logging-hygiene
`[warning]`).

## Step 6 — Admin API + settings UI (T011–T016)

`upsert_config` encrypts a supplied key (keep-on-empty) and persists the flag
(defaulting to the stored value when omitted); `build_response` fills the AI
fields. Regenerate `shared/remote-types.ts` (`pnpm run remote:generate-types`).
`SlackSettingsSection.tsx` adds the toggle, masked key field, disclosure copy,
and manifest history scopes + re-install note. The hook/api carry the fields
generically. i18n uses inline `t()` fallbacks, matching the base feature (no new
locale keys — passes `check-unused-i18n-keys`).

## Step 7 — Tests & gates (T017–T019)

Unit tests: `anthropic::prompt` caps, `IssueSummary` fixture parse, the gate,
`conversations.replies` parse. Gates: `cargo fmt`/clippy clean on remote;
`pnpm run check` (all TS type-checks pass); i18n + ui lint pass;
`cargo test --workspace`. Then the independent Codex review pass.

## Deviations from the SpecKit plan

None material. The remote type generator bin is `generate_types.rs`
(bin-named `remote-generate-types`), not `remote-generate-types.rs` — verified
against `package.json` (resolves the analyze `[warning]` on the filename). Its
DTOs were already registered, so no generator edit was needed — only the struct
fields.
