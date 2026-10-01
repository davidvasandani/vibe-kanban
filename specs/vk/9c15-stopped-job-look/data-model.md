# Data model: vk/9c15-stopped-job-look

No schema or type changes.

Fields the change reads or writes:

| Record | Field | Use |
| --- | --- | --- |
| `EventBatch` | `latest_available` | regression test against the tracker cursor |
| `JobSummary` (inventory) | `execution_id`, `worker_job_id`, `request_digest` | identity match against `execution_worker_jobs` |
| `JobSummary` | `last_sequence` | must equal `latest_available` (same journal generation) |
| `JobSummary` | `state`, `terminal` | mapped to dispatch state + process status |
| `execution_worker_jobs` | `output_complete` | set false on regression |
| `execution_worker_jobs` | `dispatch_state`, `terminal_evidence`, `completed_at` | written by the existing terminal block |
| `execution_processes` | `status`, `exit_code` | `Interrupted` (or `Indeterminate`) via existing helpers |
