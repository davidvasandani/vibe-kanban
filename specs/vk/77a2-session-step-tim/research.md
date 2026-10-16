# Research

The screenshot's repeated 9:19 is processCreatedAt fallback in ConversationTimestamp.
Claude and most other normalizers explicitly emit timestamp: None. MsgStore is
the shared patch publication boundary. Existing cache_finished_execution writes
settled patch values to the normalized sidecar at completion. Raw-log writers
skip JsonPatch; thus stamping replay would be incorrect and restarts cannot
recover observation timestamps absent a sidecar. Opt in at fresh execution
creation, never at generic normalizer startup (also used during reattachment).

Do not add browser receipt times: refresh would alter history. Do not globally
stamp ConversationPatch: historical normalizers invoke it too.
