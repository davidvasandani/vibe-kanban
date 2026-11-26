# Research
- Durable wakeups dispatch through ordinary follow-up execution; no timer-specific frontend update API is needed.
- Server session discovery uses subscribe_before_snapshot and forwards session-filtered process patches.
- The history hook returns early after initial load and only fetches terminal entries when the prior status was running. Terminal-first discoveries are missing.
- Reuse previousStatusMapRef as observed-ID evidence, preserving bounded initial history. Avoid refetching the whole conversation or adding timer polling.
