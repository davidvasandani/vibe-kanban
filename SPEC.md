# Workspace Browser verification handoff

## Problem and evidence
On 2026-10-18 the user observed SectigoStore checkout verification repeatedly spin and reset during human takeover in workspace 7fd6799a-9669-4f35-a643-f0c451ff5533, browser session 9d2fe322-c2b0-43ab-8311-83019d4eaa15. The reported environment exposed HeadlessChrome/149 and navigator.webdriver=true. Scripts returned 200. Cross-origin performance status 0 is not proof of a failed request. Widget teardown and unexpected-source warnings do not establish the rejection cause. Reproduce where accessible and label any unverified evidence explicitly.

## Scope
Vibe Kanban browser driver, session/control transitions, UI handoff, sanitized diagnostics and tests. Hosting changes only in homelab/modules/vibe-kanban-rebuild.nix if needed. No other services, HP Mini or AMT configuration changes.

## Required behavior
Provide an explicit local-browser fallback for verification unsupported by hosted automation. Evaluate headed operation but do not promise compatibility merely because Chrome has a display. Explain that the local browser has its own cookies and cart and how to reconstruct checkout. Preserve the hosted session and cart during handoff. Never copy cookies, challenge tokens or payment data.

Agent leases remain exclusive. Human takeover and release change ownership without relaunch, navigation, reload or widget manipulation. Inspect actual CDP input delivery, frame/message handling, cookie/profile lifetime and network continuity.

Diagnostics must be bounded and sanitized by construction: classify known console warnings and request failures without retaining raw console arguments, URLs with secrets, request/response bodies, headers, cookies, tokens or payment details. Distinguish actual request failure events from resource timing status 0.

## Verification and acceptance
Cover fallback safety, unchanged session identity and navigation during transitions, lease exclusion, and diagnostic redaction with regression tests. Use only provider test keys for automated challenge fixtures, never solve production challenges through automation. A human must exercise the real checkout without purchasing; record whether checkout is reached or the actionable local fallback is used. Do not claim this manual check passed without human evidence.

## Delivery
Follow the supplied WikiLLM and SpecKit stages in order, run an independent Codex review, record reusable knowledge, then open and merge a PR after required validation. Record environment blockers and unresolved acceptance criteria honestly.
