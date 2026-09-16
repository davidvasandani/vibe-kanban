# Contract: Follow-up Process Reconciliation

1. Reconciliation accepts only a complete `ExecutionProcess` returned by a
   successful follow-up request.
2. The process `session_id` must equal the provider's current session ID.
3. Projection identity is `ExecutionProcess.id`; duplicate delivery never
   creates a second process or conversation turn.
4. Before live delivery, the response value participates in the same visible/all
   process projection consumed by conversation history.
5. When live state contains the same ID, the live value wins immediately and
   owns all subsequent replacement/removal behavior.
6. A session-scope change clears response-derived state.
7. Failed requests and new-session creation do not invoke this reconciliation
   contract.
8. The callback does not clear drafts or attachments; existing composer success
   handling remains the sole owner of that behavior.
