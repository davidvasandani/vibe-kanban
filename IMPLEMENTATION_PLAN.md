# Implementation Plan: Executor-Neutral MCP Restart

1. Replace the toolbar’s `useMcpRefresh` dependency with the existing common
   follow-up and queued-follow-up lifecycle.
2. Add a small tested orchestration helper that starts stopped sessions,
   confirms running-session queueing, and preserves existing queued user input.
3. Compute running state from the selected session’s running coding-agent
   process rows.
4. Use the shared confirmation dialog and truthful restart copy.
5. Run the focused unit suite, web-core typecheck, formatting, and independent
   review.
6. Record the reusable lifecycle rule in the project knowledge base.
