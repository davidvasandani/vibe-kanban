1. Trace dispatch refusal, coordinator handoff, startup continuation and user stop paths.
2. Persist a bounded recovery claim for definitively refused work and its continuation identity. Preserve the session and action payload.
3. Verify worker readiness, append the failed attempt and verification to session history, then dispatch through the existing execution owner.
4. Reconcile interrupted recovery claims against worker execution evidence. Surface unknown outcomes and exhausted attempts with a next action.
5. Exercise two coordinator restarts, event replay, inactive controls, and successful and failed operations containing npm announcements.
6. Review the diff, document verified behavior, and merge after verification.
