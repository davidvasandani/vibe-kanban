# Implementation plan: graceful degradation for refused executions

1. **Typed refusal (services).** In `cluster/scheduler.rs`, add
   `worker_node_id: Option<Uuid>` to `WorkerUnavailable` and a
   `Refusing { detail }` reason. Give the reason a stable snake_case code.
   `dispatch_liveness` fills in the node id. Add
   `WorkerClientError::is_unavailable()` (a `Rejected` 503).
2. **Dispatch mapping (local-deployment).** In `dispatch_execution`, keep the
   failure evidence as it is, but return
   `ContainerError::WorkerUnavailable { reason: Refusing }` for a 503
   rejection. Other errors keep the existing `Other` message.
3. **API contract (server).** Change `ApiError::WorkerUnavailable` to carry a
   `WorkerUnavailableErrorData` (ts-rs). Respond with 503 and
   `error_with_data_and_message`. Export the type in `generate_types.rs` and
   regenerate `shared/types.ts`.
4. **Rust tests.** Message and reason for `Refusing`; the 503 body contains
   `error_data` and `message`; liveness carries the node id.
5. **API client (web-core).** `handleApiResponse` passes `error_data` from
   non-2xx JSON bodies into `ApiError`. Add an `isWorkerUnavailableError`
   guard.
6. **Recovery logic (web-core, pure).** `workerRecovery.ts`: rank
   alternatives (eligible, not refusing, not the refused server, lowest
   load), compute backoff delays (5 s doubling to 60 s), and define the
   10-minute deadline. Add Vitest coverage.
7. **Recovery hook.** `useWorkerUnavailableRecovery`: holds state per session
   (failed prompt, refused worker, deadline, phase), polls the worker list on
   backoff, auto-resends to the recovered original, moves and resends on
   request, and handles dismiss and reset when the session changes.
8. **Chat box UI.** `useSessionSend` exposes the raw failure. The container
   starts recovery on a typed refusal and renders a `serverRecoveryNotice`
   banner in `SessionChatBox` (ui package) with alternatives, a waiting
   countdown, an expired state, retry and dismiss. Successful resends run the
   normal post-send cleanup.
9. **i18n.** Add keys under `tasks.conversation.serverRecovery` in all 7
   locales.
10. **Verify.** Run `pnpm install --frozen-lockfile`, `generate-types`,
    `cargo test` (services, server, local-deployment scoped), `pnpm run check`,
    `pnpm run lint`, Vitest for the new files, and `pnpm run format`.
