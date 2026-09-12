# Research

The normal `useSessionSend` path already launches a fresh executor-specific
follow-up while preserving conversation identity. The normal
`useSessionQueueInteraction` path already hands queued input to execution
finalization, which launches that same fresh follow-up after a running turn.
Using these shared paths supports every executor without adding vendor restart
APIs. Codex live reload is therefore not used as the correctness boundary.
