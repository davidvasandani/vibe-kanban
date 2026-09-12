# Contract: JSON Patch WebSocket Recovery

- Initial object allocation does not set authoritative readiness.
- A `Ready` message for the current endpoint sets readiness and resets unhealthy
  retry pressure.
- More than six consecutive pre-Ready failures surfaces `Connection failed`.
- Same-endpoint failure after Ready retains the last valid snapshot and
  initialized rendering while retrying.
- Open without Ready does not reset exponential backoff.
- A decoded relay close is delivered once with its server code/reason even when
  the underlying browser socket must be closed without that reserved code.
- Any unexpected/authority-loss close schedules reconnect/resnapshot.
