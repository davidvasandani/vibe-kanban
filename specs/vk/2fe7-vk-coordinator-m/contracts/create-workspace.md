# Contract: Create and Start Workspace Placement

The existing create-and-start workspace JSON body gains one field:

```json
{
  "run_on_coordinator": false,
  "requested_worker_node_id": null
}
```

## Compatibility

- Omitted `run_on_coordinator` deserializes as `false`.
- `false` plus a null worker ID preserves automatic scheduling.
- `false` plus a worker UUID preserves explicit worker placement.
- `true` plus a null worker ID requests coordinator-local execution.
- `true` plus a worker UUID returns HTTP 400 with a message identifying the contradictory fields.

The response contract is unchanged.
