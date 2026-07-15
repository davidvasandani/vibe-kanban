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
  definition: McpServerDefinition;
  assignments: BaseCodingAgent[];
};

export type SharedMcpDraftState = {
  servers: SharedMcpDraftServer[];
  conflicts: SharedMcpConflict[];
};

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
      definition: server.definition,
      assignments: server.assignments,
      native_overrides: {},
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
      executor === BaseCodingAgent.CODEX
    );
  const server: SharedMcpDraftServer = {
    name: conflict.name,
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
