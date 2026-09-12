# Contract: Final Output and Execution Finalization

- Final assistant output is answer evidence, not exit evidence.
- Normal terminal event/exit wins immediately and cancels reconciliation.
- Final output with no terminal event starts a 45-second evidence window.
- Positive owner-specific liveness prevents premature classification.
- No positive liveness plus no stronger terminal evidence yields
  `indeterminate` after required preservation.
- Proven success, failure, or interruption maps to its exact status.
- Terminal persistence is boundedly retried and failures are structured,
  execution-scoped diagnostics.
- The authoritative execution stream publishes the terminal row, causing the
  composer to derive Send without refresh.
