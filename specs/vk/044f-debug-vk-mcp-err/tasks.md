# Tasks: Actionable MCP Authentication-Response Diagnostics

**Feature**: `specs/vk/044f-debug-vk-mcp-err/`  
**Task**: `vk/044f-debug-vk-mcp-err`

Tasks are ordered by dependency layer. Tasks marked `[P]` may run in parallel
after their dependencies complete because they touch independent files, are
read-only, or are validation-only. Most implementation tasks touch
`crates/executors/src/mcp_test.rs`, so they are intentionally sequential.

## Layer 0 - Baseline and Orientation

- [x] T001 Read the feature inputs and current probe code before editing:
      `specs/vk/044f-debug-vk-mcp-err/spec.md`,
      `specs/vk/044f-debug-vk-mcp-err/plan.md`,
      `specs/vk/044f-debug-vk-mcp-err/contracts/probe-diagnostics.md`,
      `specs/vk/044f-debug-vk-mcp-err/research.md`, and
      `crates/executors/src/mcp_test.rs`. Confirm the change remains confined
      to the executors MCP probe unless a test exposes a necessary adjacent
      change.

- [x] T002 Establish the focused backend baseline from the repo root with
      `cargo test -p executors mcp_test`. Record any pre-existing failure before
      feature edits and avoid masking unrelated failures.
      Depends on T001.

## Layer 1 - Test Fixtures and Contract Coverage

- [x] T003 Extend the loopback HTTP fixture support in
      `crates/executors/src/mcp_test.rs` so tests can assert whether a redirect
      target listener was contacted. Keep fixtures local-only and synthetic.
      Depends on T002.

- [x] T004 Add a focused HTTP test for a `302` response with
      `WWW-Authenticate` and a `Location` pointing at the second listener.
      Assert `McpServerTestStatus::AuthRequired`, exact challenge preservation
      in `www_authenticate`, an `HTTP 302` diagnostic, and zero contact with the
      redirect target.
      Depends on T003.

- [x] T005 Add a focused HTTP test for a `302` response without
      `WWW-Authenticate`. Assert `McpServerTestStatus::Failed`, no
      `www_authenticate`, an `HTTP 302` diagnostic, and a sanitized destination
      summary that omits query string, fragment, and userinfo.
      Depends on T003.

- [x] T006 Add a focused HTTP test for a `200 text/html` non-MCP body. Assert
      `McpServerTestStatus::Failed`, no `www_authenticate`, diagnostic context
      containing `HTTP 200`, the response content type, JSON parsing context,
      and a bounded body preview, while not returning only
      `invalid JSON response: expected value at line 1 column 1`.
      Depends on T003.

- [x] T007 Add a focused legacy SSE test for a challenged redirect on the
      initial SSE GET path. Assert the same no-follow and `auth_required`
      behavior as the Streamable HTTP challenged redirect.
      Depends on T003.

- [x] T008 [P] Perform the spec-required read-only deployment/configuration
      inspection for the documented MCP endpoint, using only repository files
      and safe read-only commands. Capture any finding for the implementation
      handoff; do not read, print, modify, rotate, or request credentials.
      Depends on T001.

## Layer 2 - Probe Implementation

- [x] T009 Build the shared probe `reqwest::Client` in
      `test_mcp_servers` with `reqwest::redirect::Policy::none()`. Handle the
      builder error by returning stable failed results for all supplied servers
      instead of panicking. Preserve unsupported-server behavior where practical.
      Depends on T004, T005, T006, and T007.

- [x] T010 Generalize `http_status_error` in
      `crates/executors/src/mcp_test.rs`: capture a readable, non-empty
      `WWW-Authenticate` before consuming the body; keep `401` and `403`
      classified as `ProbeError::AuthRequired`; classify `3xx` with a
      challenge as `ProbeError::AuthRequired`; and keep all other non-success
      statuses as `ProbeError::Other`.
      Depends on T009.

- [x] T011 Add redirect diagnostic support to the shared status helper. Include
      `HTTP <status>` and the bounded body snippet as today, and for redirects
      with a `Location`, add only a sanitized destination summary containing
      scheme, host, optional port, and path for absolute URLs or path only for
      relative URLs. Omit query, fragment, userinfo, arbitrary headers, and
      unparseable raw values.
      Depends on T010.

- [x] T012 Improve the invalid non-SSE `2xx` path in `http_send`: preserve
      response status and content type before consuming the body; if JSON
      parsing fails, return a failed diagnostic containing `HTTP <status>`,
      content type when present, serde parsing context, and the existing
      200-character `snippet()` preview. Do not include configured request
      headers or arbitrary response headers.
      Depends on T011.

- [x] T013 Ensure Streamable HTTP POSTs, the legacy SSE initial GET, and legacy
      SSE message POSTs all use the same no-redirect client and
      `http_status_error` classification path, with no duplicated redirect or
      authentication logic.
      Depends on T012.

## Layer 3 - Regression Coverage

- [x] T014 Confirm existing focused tests still cover current `401`, `403`,
      `500`, connection-refused, unsupported, stdio failure, valid JSON
      handshake, and valid SSE handshake behavior. Add narrow regression
      assertions only if a listed behavior is not already covered in
      `crates/executors/src/mcp_test.rs`.
      Depends on T013.

- [x] T015 Add or adjust sanitization assertions so diagnostics never include
      configured `Authorization`, `CF-Access-Client-*`, cookies, request header
      values, redirect query strings, redirect fragments, redirect userinfo, or
      unbounded body content.
      Depends on T014.

## Layer 4 - Validation

- [x] T016 Run the focused executor MCP tests:
      `cargo test -p executors mcp_test`.
      Depends on T015.

- [x] T017 [P] Run the relevant backend check lane:
      `pnpm run backend:check`.
      Depends on T015.

- [x] T018 Run repository formatting as required by `AGENTS.md`:
      `pnpm run format`.
      Depends on T016 and T017.

- [x] T019 Run a final focused diff review against every acceptance criterion
      in `spec.md` and the contract table in `contracts/probe-diagnostics.md`.
      Confirm no frontend, generated TypeScript, schema, route, dependency, or
      credential-handling changes were introduced.
      Depends on T018.

- [x] T020 Record the validation results and any read-only deployment/config
      finding from T008 in the implementation handoff or PR notes. Explicitly
      call out any residual operator-only credential or production action.
      Depends on T008 and T019.

## Parallelization Notes

T008 can run in parallel with the test and implementation work after T001
because it is read-only and independent. T017 can run in parallel with T016
after T015, but formatting and final review should run after both validation
commands complete. Tasks T003-T015 all touch the same Rust module and should be
sequenced to avoid merge conflicts and drifting diagnostics.
