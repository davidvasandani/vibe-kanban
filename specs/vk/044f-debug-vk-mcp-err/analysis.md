# Analysis: MCP authentication-response diagnostics

**Inputs checked**: [`spec.md`](spec.md), [`plan.md`](plan.md),
[`tasks.md`](tasks.md), [`contracts/probe-diagnostics.md`](contracts/probe-diagnostics.md),
[`research.md`](research.md), and `.specify/memory/constitution.md` v0.10.0.

## Findings

- **High - Legacy SSE message POST redirect behavior is required but not
  directly covered.**  
  `spec.md` FR-1 requires redirect following to be disabled for Streamable HTTP
  POSTs, the legacy SSE GET, and legacy SSE message POSTs. FR-12 also requires
  legacy SSE to gain the same redirect visibility and safe failure diagnostics.
  `plan.md` says the common client/status helper should cover legacy SSE POSTs,
  but its test strategy only names "legacy SSE challenged redirect" and frames
  it around the initial SSE behavior. `tasks.md` T007 explicitly tests only the
  initial SSE GET path, while T013 is an implementation inspection task, not a
  regression test. This leaves a realistic gap where `sse_post()` could keep
  following or misclassifying redirects without a focused test failing. Add a
  task for challenged and/or unchallenged redirect behavior on the legacy SSE
  message POST path after the endpoint event is received.

- **Medium - Deployment inspection task no longer satisfies the spec's deployed
  endpoint check.**  
  `spec.md` FR-17 says the deployed endpoint and repository deployment
  configuration SHOULD be inspected read-only. `tasks.md` T008 narrows this to
  "using only repository files and safe read-only commands," which can inspect
  repository configuration but cannot verify the credential-free live boundary
  response from the deployed endpoint. This is inconsistent with the spec's goal
  of distinguishing probe behavior from an external deployment or credential
  provisioning defect. Reword T008 to include a credential-free, no-secret,
  read-only live endpoint check when network access is available, and to record
  if environment restrictions prevent that check.

- **Medium - Client-builder failure handling may regress unsupported results.**  
  `spec.md` FR-13 requires unsupported executor results not to regress.
  `tasks.md` T009 says a reqwest client builder error should return stable
  failed results for all supplied servers and only "preserve unsupported-server
  behavior where practical." That weakens the MUST in FR-13. Even though client
  builder failure is expected to be rare, the task should require unsupported
  configs to remain `unsupported` by normalizing entries before assigning
  builder-error failures to runnable HTTP/SSE targets.

- **Medium - The planned body-preview helper can exceed the stated display
  limit.**  
  `spec.md` FR-7 and the acceptance criteria require the body preview to be no
  more than 200 displayed characters. `plan.md` and `tasks.md` direct reuse of
  the existing `snippet()` preview as a 200-character bound. The current helper
  takes 200 characters and appends an ellipsis for longer strings, so the
  displayed diagnostic preview can become 201 characters. Either change the
  implementation task to make the total displayed preview length at most 200, or
  relax the spec to "200 response characters plus truncation marker." The
  current artifacts contradict each other.

- **Low - Plan assumes existing HTTP/SSE success tests that do not appear to
  exist in the current probe test module.**  
  `spec.md` FR-10, FR-11, and FR-16 require successful Streamable HTTP JSON and
  SSE behavior to keep working and be covered by focused tests. `plan.md` says
  to retain existing successful protocol tests, but the current
  `crates/executors/src/mcp_test.rs` visible tests cover stdio success and HTTP
  failure/auth cases, not successful HTTP JSON or legacy/Streamable SSE
  handshakes. `tasks.md` T014 partially mitigates this by requiring narrow
  tests if missing. Prefer making T014 explicit: add valid HTTP JSON and valid
  HTTP/SSE response tests unless they already exist in the final branch.

## Coverage Notes

- The main backend-only scope is consistent with NF-3 and the existing result
  contract; no frontend or generated type change is required by the artifacts.
- The redirect sanitization requirements are consistently stated across the
  spec, plan, contract, and tasks, aside from the SSE message POST coverage gap.
- Secret-handling constraints are present in the spec and tasks and align with
  constitution principles IX and XI.

## Constitution Assessment

No open constitution violation is inherent in the proposed architecture. The
items above should be resolved before implementation to fully satisfy
constitution II ("Test the contract"), VI ("Don't rebuild what shipped"), IX
("External agent protocols are defensive contracts"), and XI ("Diagnostics are
evidence, not decoration").
