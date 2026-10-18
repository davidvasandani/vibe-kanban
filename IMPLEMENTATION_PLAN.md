# Implementation plan

1. Complete ordered SpecKit constitution, specification, clarification, plan, tasks and analysis using the workspace command files and their task pointer.
2. Inspect the existing browser launch, CDP input, ownership transitions, profiles and frame transport. Verify Cloudflare support guidance and attempt a read-only inspection of the reported session without taking it over.
3. Add local-browser handoff guidance to BrowserPanelContainer, with safe HTTP(S) links, no credential transfer, and explicit instructions to recreate the cart locally. Keep the hosted page and control untouched.
4. Classify console diagnostics at CDP ingestion and collect actual Network.loadingFailed events in a bounded ring, with no raw arguments, headers, URL, request bodies or payment data. Add regression tests.
5. Extend session/control regression coverage to prove ownership changes preserve driver state and reject stale commands. Add a provider-test-key fixture if browser automation is available.
6. Install locked dependencies; format and run targeted tests, type checks and lint. Record blockers rather than claiming skipped checks passed.
7. Have a human verify the real checkout or fallback without purchasing. Record live observations separately from reported evidence.
8. Run independent Codex review, address findings, record reusable knowledge and commit it. Open and merge the task PR only when validation and review support readiness.
