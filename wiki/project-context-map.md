# Project Context Map (monorepo service → path → IaC)

Reusable pattern for giving a freshly-spawned VK issue its scope when the
project is a monorepo (and may span several repos). Shipped in the **homelab**
repo as `project-context.json`; the design is repo-agnostic.

## Problem

A VK project can span multiple repos, and a repo can be a monorepo holding many
services. A newly-spawned issue starts cold: the agent doesn't know which
service directory the work targets or where that service's Infrastructure-as-Code
lives, so it re-derives scope from scattered `CLAUDE.md`/`AGENTS.md` prose (or
guesses). The map lived only in operators' heads.

## Solution shape

A single, version-controlled, machine-readable artifact at the repo root
(homelab: `project-context.json`) that the agent is pointed at.

```
ProjectContext = { project, repos: Repo[] }
Repo    = { name, monorepo: bool, url?, primary?, services: Service[] }
Service = { name, path, description?, docs?, iac: Iac[] }
Iac     = { kind: "nix" | "terragrunt", path, host? }
```

`repos[]` models the project as a *logical* grouping that may span repos — one
entry per repo, `monorepo` flagged. A repo you enumerate but haven't broken down
yet carries `services: []`.

## Decisions that generalize

- **JSON + `jq`, not YAML.** Zero new dependency (`jq` is already available;
  `yq`/`yamllint`/PyYAML were not) and it matches repos that already hand-author
  JSON config. A config file doesn't justify a new parser dependency.
- **Empty list means "no IaC", never a fabricated path.** A library/persona dir
  with no infra uses `"iac": []`. Don't invent a `kind: "none"` with a dummy
  path — that reintroduces the fabricated-location problem.
- **Single source of truth.** The mapping lives *only* in the JSON; `CLAUDE.md`/
  `AGENTS.md` get a *pointer*, not a copy, so the two can't drift. Surface it by
  referencing the file from the docs VK already injects — **no VK product change
  needed**.
- **IaC is a list.** A service commonly has both a runtime module (homelab:
  `modules/<svc>.nix` wired into a `hosts/*` host) and/or a cloud stack
  (`terragrunt/environments/*`). Ground each entry with `grep -rl modules/<svc>
  hosts`, not memory.

## Validation — the part reviewers break

Guard the file with a CI check (homelab pattern: a `ci/check-*.sh` bash script +
a hosted-runner workflow that fails loudly). It must assert the JSON parses and,
crucially, that **every declared `Service.path` and `Iac.path` exists on disk** —
that's what turns "documentation" into a drift-detecting invariant.

Gotchas found in review (all caused silent **false-passes** — the check
reporting success while the map was broken):

1. **Empty/missing path skipped, not flagged.** If you emit paths as TSV and
   iterate, a missing/typo'd `path` key renders as an empty field; `[ -n "$p" ]
   || continue` then *skips* it. Add an explicit jq check that every path is a
   non-empty string **before** the existence loop.
2. **Iterate with process substitution, not a pipe.** `while … done < <(jq …)`
   keeps the loop in the main shell so `fail=1` / `err()` actually affect the
   exit code. `jq … | while …` runs the loop in a subshell and the failure flag
   is lost — the check exits 0 no matter what.
3. **Enforce `additionalProperties: false` at *every* level.** A JSON Schema is
   documentation only unless something runs it. With hand-rolled jq checks, a
   misspelled *optional* key (`primary`→`primry`, `host`→`hosts`) silently
   passes and drops its meaning. Reject unknown keys at top / repo / service /
   iac level via `keys_unsorted[] | select(IN(<allow-list>)|not)`. Required-key
   typos usually surface as type failures, but optional ones don't.
4. **Enforce documented cardinality** (e.g. "at most one repo `primary: true`").

Verify by actually breaking entries: for each guard, mutate the file with `jq`,
run the check, confirm non-zero exit + a clear message, then restore.

## Rejected alternatives

- YAML (needs `yq`/PyYAML dependency), a `.nix` attrset (not readable without
  evaluating Nix), per-service files (scatters the map), a runtime JSON-Schema
  validator as the gate (adds a dependency and still can't verify paths exist),
  and modifying VK to parse/inject the file (out of scope; the doc pointer
  suffices).

## Contributed by

- vk/4931-vk-project-conte
