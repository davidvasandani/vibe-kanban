# Existing Contract: Project Detail

`GET /v1/projects/{project_id}`

- Authentication: existing remote API authentication.
- Success: HTTP 200 with the generated `Project` JSON shape.
- Confirmed absence/inaccessibility as exposed by the route: HTTP 404 maps to
  frontend `null`.
- Other non-success response: frontend throws the parsed remote error.

The feature does not change this server contract. It adds a typed `getProject`
consumer in `packages/web-core/src/shared/lib/remoteApi.ts`.
