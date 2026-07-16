# Data Model: CLI tool OAuth login

No persistent database schema is introduced.

## Compile-time catalog additions

`CliToolCatalogEntry` gains an optional authentication strategy:

- login executable arguments (the executable is always `binary_name`),
- probe arguments and a result classifier,
- user-facing unsupported reason when no safe persistent login exists.

## API state

`CliToolStatus` gains:

- `auth_state`: `authenticated | unauthenticated | unknown | unsupported`,
- `auth_message`: optional non-secret explanation,
- `login_supported`: boolean derived from catalog strategy.

## Runtime session

An active login session contains:

- tool id and generated PTY session id,
- start/deadline timestamps,
- resolved executable path and catalog arguments,
- lifecycle state: `starting | active | succeeded | failed | cancelled | timed_out`,
- output receiver and child exit receiver.

Sessions are in-memory, at most one per tool in a server/machine process, and
removed on all terminal paths. Output is streamed only and not retained.
