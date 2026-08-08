# API Contract: Affinity Migration Session Transfer

The existing workspace affinity mutation keeps its request contract. Its
response gains a distinct pre-stop outcome:

```text
session_transfer_failed {
  operation_id,
  phase,
  category,
  message,
  remediation,
  safe_details
}
```

When this outcome is returned:

- `stopped_execution_id` is absent;
- `started_execution` is absent;
- placement equals the original placement; and
- the recorded source execution remains running/indeterminate exactly as it was
  before the transfer attempt.

Categories include `missing_lineage`, `invalid_lineage`, `size_limit`,
`authorization`, `source_changed`, `checksum_mismatch`, `target_conflict`,
`verification`, `timeout`, and `transport`. Messages identify operation,
thread, and worker where safe but contain no rollout contents.

Completed retries replay the same stored outcome. Retrying a resumable ambiguous
transfer uses the same operation ID. A corrected request after a conclusive
failure uses a new operation ID under the existing affinity API rules.
