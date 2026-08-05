import { BaseCodingAgent } from 'shared/types';
import type {
  JsonValue,
  McpServerDefinition,
  SharedMcpAssignmentTestResult,
  SharedMcpConflict,
  SharedMcpConflictVariant,
  SharedMcpReadResponse,
  SharedMcpServer,
  SharedMcpServerInput,
  SharedMcpTestTarget,
} from 'shared/types';

export type SharedMcpDraftServer = {
  name: string;
  displayName: string | null;
  definition: McpServerDefinition;
  assignments: BaseCodingAgent[];
};

export type SharedMcpDraftState = {
  servers: SharedMcpDraftServer[];
  conflicts: SharedMcpConflict[];
};

export type PreconfiguredMcpServer = {
  key: string;
  entry: JsonValue;
  name: string;
  description: string;
  icon?: string;
};

export function preconfiguredMcpServers(
  value: JsonValue
): PreconfiguredMcpServer[] {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return [];
  }
  const catalog = value as Record<string, JsonValue>;
  const metadata =
    typeof catalog.meta === 'object' &&
    catalog.meta !== null &&
    !Array.isArray(catalog.meta)
      ? (catalog.meta as Record<string, JsonValue>)
      : {};

  return Object.entries(catalog)
    .filter(([key, entry]) => key !== 'meta' && entry !== undefined)
    .map(([key, entry]) => {
      const rawMeta = metadata[key];
      const meta =
        typeof rawMeta === 'object' &&
        rawMeta !== null &&
        !Array.isArray(rawMeta)
          ? (rawMeta as Record<string, JsonValue>)
          : {};
      return {
        key,
        entry,
        name: typeof meta.name === 'string' ? meta.name : key,
        description:
          typeof meta.description === 'string' ? meta.description : '',
        icon: typeof meta.icon === 'string' ? meta.icon : undefined,
      };
    });
}

/**
 * Picks the identifier for a newly instantiated catalog template, given the
 * names already in the draft.
 *
 * Catalog templates describe a *kind* of server, not one instance of it — a user
 * may want two Slack workspaces, or a Gmail server per mailbox. Since the
 * template's own key can only be used once, later instances get `_2`, `_3`, …
 *
 * The `_` separator is load-bearing. These names are protocol identifiers
 * written into agents' native config files, not display labels, and the backend
 * validates them against `^[a-zA-Z0-9_-]+$` (`is_valid_server_identifier` in
 * `crates/executors/src/shared_mcp_config.rs`). A space or `(2)` would be
 * rejected on save, or silently rewritten by `suggested_server_identifier`.
 *
 * Returning a name already in `existing` would be worse than an error:
 * `setServer` de-duplicates by name, so a collision replaces the earlier server
 * rather than reporting a conflict.
 */
/**
 * Every logical server name the draft has spoken for.
 *
 * A name lives in `servers` **or** `conflicts`, never both: the backend routes a
 * name whose definitions diverge across agents into `conflicts` instead of
 * `servers`. So `draft.servers` alone is not the set of taken names, and reusing
 * a conflicting name would silently bind a new definition to a conflict the user
 * has not resolved yet.
 */
export function takenServerNames(state: SharedMcpDraftState): string[] {
  return [
    ...state.servers.map((server) => server.name),
    ...state.conflicts.map((conflict) => conflict.name),
  ];
}

export function nextAvailableServerName(
  key: string,
  existing: readonly string[]
): string {
  const taken = new Set(existing);
  if (!taken.has(key)) return key;
  let suffix = 2;
  while (taken.has(`${key}_${suffix}`)) suffix += 1;
  return `${key}_${suffix}`;
}

export function sharedMcpSnapshot(state: SharedMcpDraftState): string {
  return JSON.stringify({
    servers: [...state.servers]
      .map((server) => ({
        ...server,
        assignments: [...server.assignments].sort(),
      }))
      .sort((a, b) => a.name.localeCompare(b.name)),
    conflicts: state.conflicts,
  });
}

export function draftFromSharedRead(
  response: SharedMcpReadResponse
): SharedMcpDraftState {
  return {
    servers: response.servers.map((server) => ({
      name: server.name,
      displayName: server.display_name,
      definition: server.definition,
      assignments: server.assignments.map((assignment) => assignment.executor),
    })),
    conflicts: response.conflicts,
  };
}

export function inputsFromDraft(
  state: SharedMcpDraftState
): SharedMcpServerInput[] {
  return state.servers
    .filter((server) => server.definition.transport !== 'unknown')
    .map((server) => ({
      name: server.name,
      display_name: server.displayName,
      definition: server.definition,
      assignments: server.assignments,
      native_overrides: {},
    }));
}

export function draftServersFromInputs(
  servers: SharedMcpServerInput[]
): SharedMcpDraftServer[] {
  return servers.map((server) => ({
    name: server.name,
    displayName: server.display_name,
    definition: server.definition,
    assignments: server.assignments,
  }));
}

export function removedServerNames(
  read: SharedMcpReadResponse | null,
  draft: SharedMcpDraftState
): string[] {
  const draftNames = new Set(draft.servers.map((server) => server.name));
  return Array.from(
    new Set([
      ...(read?.servers ?? [])
        .filter((server) => !draftNames.has(server.name))
        .map((server) => server.name),
      ...(read?.conflicts ?? [])
        .filter(
          (conflict) =>
            !draft.conflicts.some((item) => item.name === conflict.name) &&
            !draftNames.has(conflict.name)
        )
        .map((conflict) => conflict.name),
    ])
  );
}

export function resolveConflictVariant(
  state: SharedMcpDraftState,
  conflict: SharedMcpConflict,
  variant: SharedMcpConflictVariant
): SharedMcpDraftState {
  if (variant.definition.transport === 'unknown') return state;
  const supportsSelectedDefinition = (executor: BaseCodingAgent) =>
    !(
      variant.definition.transport === 'sse' &&
      (executor === BaseCodingAgent.CODEX || executor === BaseCodingAgent.GROK)
    );
  const server: SharedMcpDraftServer = {
    name: conflict.name,
    displayName: conflict.display_name,
    definition: variant.definition,
    assignments: Array.from(
      new Set(
        conflict.variants.flatMap((item) =>
          item.assignments.map((assignment) => assignment.executor)
        )
      )
    ).filter(supportsSelectedDefinition),
  };
  return {
    servers: [
      ...state.servers.filter((item) => item.name !== conflict.name),
      server,
    ].sort((a, b) => a.name.localeCompare(b.name)),
    conflicts: state.conflicts.filter((item) => item.name !== conflict.name),
  };
}

export function testKey(serverName: string, executor: BaseCodingAgent): string {
  return `${serverName}::${executor}`;
}

export function testTargetsForDraft(
  state: SharedMcpDraftState,
  serverName?: string
): SharedMcpTestTarget[] {
  return state.servers
    .filter((server) => serverName === undefined || server.name === serverName)
    .flatMap((server) =>
      server.assignments.map((executor) => ({
        server_name: server.name,
        executor,
      }))
    );
}

export function indexAssignmentTests(
  results: SharedMcpAssignmentTestResult[]
): Record<string, SharedMcpAssignmentTestResult> {
  return Object.fromEntries(
    results.map((result) => [
      testKey(result.server_name, result.executor),
      result,
    ])
  );
}

export function updateServerDefinitionValue(
  definition: McpServerDefinition,
  value: JsonValue
): McpServerDefinition {
  return {
    ...definition,
    value,
    representable_in_form: true,
  };
}

export function definitionFromEntry(entry: JsonValue): McpServerDefinition {
  const obj =
    typeof entry === 'object' && entry !== null && !Array.isArray(entry)
      ? (entry as Record<string, JsonValue>)
      : {};
  const transport =
    typeof obj.command === 'string' || Array.isArray(obj.command)
      ? 'stdio'
      : typeof obj.url === 'string' || typeof obj.httpUrl === 'string'
        ? obj.type === 'sse'
          ? 'sse'
          : 'http'
        : 'unknown';

  const value =
    transport === 'stdio'
      ? {
          command: obj.command,
          args: obj.args,
          env: obj.env ?? obj.environment,
        }
      : transport === 'http' || transport === 'sse'
        ? {
            url: obj.url ?? obj.httpUrl,
            headers: obj.headers,
          }
        : entry;

  return {
    transport,
    value: stripUndefined(value),
    representable_in_form: transport !== 'unknown',
  };
}

function stripUndefined(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(stripUndefined);
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value).filter(([, v]) => v !== undefined)
  ) as JsonValue;
}

export function mergeOAuthRefresh(
  current: SharedMcpDraftState,
  refreshed: SharedMcpReadResponse,
  serverName: string,
  executor: BaseCodingAgent
): SharedMcpDraftState {
  const freshServer = refreshed.servers.find(
    (server) => server.name === serverName
  );
  if (!freshServer) return current;
  const freshAssignment = freshServer.assignments.find(
    (assignment) => assignment.executor === executor
  );
  if (!freshAssignment) return current;

  return {
    ...current,
    servers: current.servers.map((server) =>
      server.name === serverName
        ? {
            ...server,
            definition: freshServer.definition,
            assignments: Array.from(
              new Set([...server.assignments, freshAssignment.executor])
            ),
          }
        : server
    ),
  };
}

export function compatibilityReason(
  server: SharedMcpServer,
  executor: BaseCodingAgent
): string | null {
  return (
    server.compatibility.find((item) => item.executor === executor)?.reason ??
    null
  );
}
