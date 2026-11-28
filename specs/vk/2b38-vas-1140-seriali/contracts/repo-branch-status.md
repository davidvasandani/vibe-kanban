# Contract: GET /api/workspaces/{id}/git/status

`RepoBranchStatus` gains one additive, nullable field:

```ts
source_mirror_error: SourceMirrorFailure | null;

type SourceMirrorFailure = {
  kind: "deterministic" | "transient" | "timeout";
  message: string;          // includes git's stderr excerpt
  attempts: number;
  last_attempt_at: string;  // RFC 3339
  next_attempt_at: string;  // RFC 3339
};
```

Old clients ignore the field. It is `null` when the mirror is healthy,
disabled, or has not run since start.
