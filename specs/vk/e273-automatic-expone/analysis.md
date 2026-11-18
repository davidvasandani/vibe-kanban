# Analysis: spec, plan and tasks against the constitution

No errors. No constitution violations. Spec, plan and tasks were not changed
by this stage; the warnings are carried into implementation.

- **warning — tasks.md (T006):** FR-6's queued-message case has no named test.
  The supersession test must include "a queued message ran first".
- **warning — tasks.md (T007):** the acceptance criteria "setting off schedules
  nothing", "second identical event adds none" and "eleventh failure schedules
  nothing" need explicit tests; T007 says only "with tests".
- **warning — plan.md / data-model.md:** a configured `max_delay_secs` below
  `base_delay_secs` is not addressed. The cap must win, and T001 needs a test.
- **warning — tasks.md (T012):** the docs file is not named. Pick the page at
  implementation time and record it in the task.
- **warning — tasks.md (T011):** assumes all seven locales carry the
  neighbouring `autoResume` strings. Verify before editing.
- **info — spec.md FR-10:** restart durability relies on the existing wake-up
  recovery test; no capacity-specific restart test is planned. Acceptable
  because the row and delivery shapes are unchanged.
- **info — plan.md step 6 vs XLI:** the remediation skip reads the local log
  before reserving a guard slot. XLI's reserve-before-I/O rule concerns remote
  calls that could half-complete; a skipped event correctly reserves nothing.
- **info — plan.md step 5:** a retry due at the same moment as an agent
  wake-up is delivered in the same turn (both have a nil issue id). Attempt
  chaining still works because each delivery records the execution.
- **info — LX coverage:** evidence trigger (T001, T007), bound and chain
  (T001, T003, T007), idempotence (T003), admission-time supersession (T006),
  no unarchive and separate budget (T003), visibility (T010), config read when
  acting (T007), remediation exclusion (T008). Every clause has a task.
- **info — FR coverage:** FR-1/2 T001+T007; FR-3 T001; FR-4 T003+T007; FR-5
  T003+T006; FR-6 T005+T006; FR-7/8/9 T003; FR-10 existing; FR-11 T010; FR-12
  T002+T009+T011; FR-13 T008.
