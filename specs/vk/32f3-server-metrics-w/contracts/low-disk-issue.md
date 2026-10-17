# Contract: Resolve or Create Low-Disk Issue

`POST /api/remote/issues/low-disk` proxies to authenticated remote
`POST /v1/issues/low-disk`.

Request:

```json
{
  "project_id": "uuid",
  "node_id": "uuid",
  "hostname": "think4",
  "observed_at": "2026-08-13T12:00:00Z",
  "filesystems": [
    {
      "device": "/dev/mapper/pool-root",
      "fs_type": "ext4",
      "mount_point": "/",
      "total_bytes": 125627793408,
      "used_bytes": 119185342464,
      "available_bytes": 106954752
    }
  ]
}
```

Success payload is the standard API envelope containing a txid-covered
mutation result plus `created`. Repeated/concurrent calls for the same project
and node return the existing non-terminal issue with `created: false`.

Validation errors cover absent project context, no affected valid filesystem,
unknown/invalid node identity or metrics shape, and a reading that does not
cross effective thresholds. Authorization is identical to ordinary issue
creation for the project.

The generated body contains an observation table and permanent-remediation
checklist. It never claims cleanup or resizing has occurred.
