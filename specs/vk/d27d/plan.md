# Durable recovery implementation
In crates/local-deployment/src/container.rs preserve ExecutionDispatch.execution_id and request_digest across retries. The worker job is the durable source of ownership. Persist a retry reservation before making a replacement request. Reconstruct only inputs whose digest still matches. Do not store cleartext environment secrets in recovery evidence.

Use the typed worker refusal and a successful readiness probe to authorize one replacement. Persist human-readable diagnostics against the session execution as transitions occur. A transport failure or ambiguous completion requires reconciliation. Keep log text, including npm notices, out of dispatch decisions.

In crates/server/src/startup.rs reconcile survivor and pending recovery evidence before ordinary interrupted-session continuation. Check cancellation and session activity before launch. Exercise startup with a dedicated coordinator and durable store across two process replacements. Verify the original execution identity and count the actual effects.

The design follows the constitution's execution ownership and replay requirements and needs no deployment changes.
