# Implementation plan — browser scrolling

1. Refresh SpecKit constitution, feature specification, clarifications, technical plan and task list; analyze before code changes.
2. Add typed BrowserAction::Scroll and CDP wheel dispatch, regenerate shared types, then extend BrowserPanelContainer input handling with controlled, frame-scaled wheel events and a cancellable single-pointer touch swipe gesture. Retain existing tap keyboard behavior.
3. Add component regression tests for direction/scaling, wheel modes, tap separation, cancellation, multiple touches and stale ownership/session boundaries.
4. Format and run focused tests, frontend type checks and lint; independently review with Codex CLI and address findings.
5. Document reusable findings in the project wiki, commit, open a PR against main and merge after CI passes.
