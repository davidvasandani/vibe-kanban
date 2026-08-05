# Contract: Frontend Restart Recovery

## Stream state

For a fixed endpoint:

- the initial snapshot is created once;
- `Ready` marks that endpoint initialized;
- unexpected close sets `isConnected=false` but retains `data` and initialized state;
- reconnect success sets `isConnected=true`, clears stream error, and resets backoff;
- a clean terminal `finished` close does not reconnect.

Changing/disabling the endpoint resets snapshot, initialized, error, and retry state because it represents a different stream identity.

## Retry

Unexpected failures retry exponentially from a one-second base, capped at eight seconds, with ±20% jitter. Only a stream with no received data may promote repeated connection failures into a blocking data error.

## Workspace restart status

The two workspace streams provide combined connection state through `WorkspaceProvider`. Before initial data is loaded, normal loading UI applies. After initialization, any combined disconnect displays one fixed, additive `role="status"` message while the existing route and workspace data stay mounted. Reconnection removes the status automatically.

This status is intentionally not a crash alert: coordinator replacement is expected recovery and worker-owned agents continue independently.
