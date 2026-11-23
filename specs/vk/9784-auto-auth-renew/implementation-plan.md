# Implementation plan

1. Register a gateway auth provider with the existing reauth service during server startup, keeping server-owned deployment/OAuth context behind an async interface. Discover only bound active OAuth connections; validate names/UUID aliases against that inventory.
2. Extend reauth identity, discovery, sweep selection and execution/verification through that provider. Separate proactive renewal eligibility from auth_state so still-valid tokens remain accurately authenticated.
3. Extend gateway credential storage/refresh: persist expiry, serialize and compare versions, keep stable local capability, retain refresh grants, use existing OAuth and browser machinery for silent fallback.
4. Add bounded initialize probes and actionable authentication-error JSON; preserve transport failures as unknown. Reuse provider repair from rejected proxy requests without introducing arbitrary RPC replay after uncertain transport outcomes.
5. Update MCP instructions and Settings grouping for the new kind. Fix cancel_wakeup duplicate path capture with a scoped regression test.
6. Verify with isolated upstream tests and repository setup/format/checks; run independent Codex review and fix significant findings.
7. Record reusable knowledge, commit, open PR against main, wait for required checks and merge.
