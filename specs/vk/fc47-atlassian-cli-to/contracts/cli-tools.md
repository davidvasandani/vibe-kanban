# Contract — CLI Tools API With Atlassian CLI

No new routes are introduced. Existing local machine API routes gain one valid
tool id: `acli`.

## Tool Id

```ts
type CliToolId =
  | 'aws'
  | 'az'
  | 'op'
  | 'gam'
  | 'mgc-beta'
  | 'acli';
```

The exact generated order is not contractually significant, but `acli` must be
present after running `pnpm run generate-types`.

## List Tools

`GET /api/cli-tools`

Returns `ApiResponse<CliToolStatus[]>`. The list includes one ACLI row:

```json
{
  "id": "acli",
  "binary_name": "acli",
  "display_name": "Atlassian CLI",
  "description": "Atlassian Cloud command-line workflow text",
  "catalog_version": "1.3.22-stable",
  "supported": true,
  "unsupported_reason": null,
  "host": null,
  "app": null,
  "docs_url": "https://developer.atlassian.com/cloud/acli/guides/install-acli/"
}
```

On unsupported hosts, `supported` is `false` and `unsupported_reason` carries
the existing platform message. Host and app fields follow current behavior.

## Install Or Update

`POST /api/cli-tools/acli/install`

`POST /api/cli-tools/acli/update`

Both routes call the same existing install path and return
`ApiResponse<CliToolStatus>`.

Expected success semantics:

- selected artifact matches host platform;
- artifact SHA-256 matches the pinned checksum;
- extracted executable is exposed as app-owned `cli-tools/bin/acli`;
- response `app.version` is `1.3.22-stable`;
- response `app.outdated` is `false`.

Expected failure semantics:

- unsupported host returns an in-band `ApiResponse::error` message;
- download, checksum, extraction, or promotion failure returns an in-band
  `ApiResponse::error` message;
- no partial app-owned `acli` is exposed on spawned-agent PATH.

## Remove

`DELETE /api/cli-tools/acli`

Returns `ApiResponse<CliToolStatus>`.

Expected semantics:

- removes app-owned `cli-tools/bin/acli` and `cli-tools/acli/`;
- does not remove or modify a host-owned `acli`;
- returned status may still report `host` if a host copy exists.

## Generated Types

`crates/server/src/bin/generate_types.rs` already exports `CliToolId`,
`HostCopy`, `AppCopy`, and `CliToolStatus`. After implementation, regenerate and
check:

```sh
pnpm run generate-types
pnpm run generate-types:check
```
