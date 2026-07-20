import { describe, expect, it } from 'vitest';
import { BaseCodingAgent } from 'shared/types';
import type { SharedMcpReadResponse } from 'shared/types';
import {
  definitionFromEntry,
  draftFromSharedRead,
  indexAssignmentTests,
  inputsFromDraft,
  mergeOAuthRefresh,
  preconfiguredMcpServers,
  removedServerNames,
  resolveConflictVariant,
  sharedMcpSnapshot,
  testKey,
  testTargetsForDraft,
} from './sharedMcpSettingsState';

const readResponse = (): SharedMcpReadResponse => ({
  profiles: [],
  servers: [
    {
      name: 'tools',
      definition: {
        transport: 'stdio',
        value: { command: 'npx' },
        representable_in_form: true,
      },
      assignments: [
        {
          executor: BaseCodingAgent.CLAUDE_CODE,
          native_name: 'tools',
          native_entry: { command: 'npx' },
          has_credentials: false,
          representable: true,
          incompatibility_reason: null,
        },
      ],
      source_kind: 'single_profile',
      native_sources: [],
      compatibility: [],
    },
  ],
  conflicts: [],
  preconfigured: {},
  read_errors: [],
});

describe('shared MCP settings state', () => {
  it('extracts catalog servers and metadata without exposing the meta entry', () => {
    expect(
      preconfiguredMcpServers({
        slack: { command: 'npx', args: ['slack-mcp-server'] },
        meta: {
          slack: {
            name: 'Slack',
            description: 'Search workspace conversations',
          },
        },
      })
    ).toEqual([
      {
        key: 'slack',
        entry: { command: 'npx', args: ['slack-mcp-server'] },
        name: 'Slack',
        description: 'Search workspace conversations',
        icon: undefined,
      },
    ]);
  });

  it('creates stable snapshots independent of assignment order', () => {
    const a = {
      servers: [
        {
          name: 'tools',
          definition: {
            transport: 'stdio' as const,
            value: { command: 'npx' },
            representable_in_form: true,
          },
          assignments: [BaseCodingAgent.GEMINI, BaseCodingAgent.CLAUDE_CODE],
        },
      ],
      conflicts: [],
    };
    const b = {
      ...a,
      servers: [
        {
          ...a.servers[0],
          assignments: [BaseCodingAgent.CLAUDE_CODE, BaseCodingAgent.GEMINI],
        },
      ],
    };
    expect(sharedMcpSnapshot(a)).toBe(sharedMcpSnapshot(b));
  });

  it('maps read responses to write inputs', () => {
    const draft = draftFromSharedRead(readResponse());
    expect(inputsFromDraft(draft)).toEqual([
      {
        name: 'tools',
        definition: {
          transport: 'stdio',
          value: { command: 'npx' },
          representable_in_form: true,
        },
        assignments: [BaseCodingAgent.CLAUDE_CODE],
        native_overrides: {},
      },
    ]);
  });

  it('keys tests by server and assignment', () => {
    const key = testKey('tools', BaseCodingAgent.GEMINI);
    expect(key).toBe('tools::GEMINI');
    expect(
      indexAssignmentTests([
        {
          server_name: 'tools',
          executor: BaseCodingAgent.GEMINI,
          result: {
            name: 'tools',
            transport: 'stdio',
            status: 'ok',
            latency_ms: 1,
            tool_count: 2,
            server_name: null,
            server_version: null,
            error: null,
            www_authenticate: null,
          },
        },
      ])[key].result.status
    ).toBe('ok');
  });

  it('builds test targets per assignment', () => {
    const draft = draftFromSharedRead(readResponse());
    expect(testTargetsForDraft(draft)).toEqual([
      { server_name: 'tools', executor: BaseCodingAgent.CLAUDE_CODE },
    ]);
  });

  it('derives canonical definitions from native entries', () => {
    expect(
      definitionFromEntry({ type: 'http', url: 'https://example.test' })
    ).toEqual({
      transport: 'http',
      value: { url: 'https://example.test' },
      representable_in_form: true,
    });
  });

  it('merges OAuth refresh into the live draft', () => {
    const current = draftFromSharedRead(readResponse());
    const refreshed = readResponse();
    refreshed.servers[0].definition.value = {
      command: 'npx',
      headers: { Authorization: 'Bearer token' },
    };
    const merged = mergeOAuthRefresh(
      current,
      refreshed,
      'tools',
      BaseCodingAgent.CLAUDE_CODE
    );
    expect(merged.servers[0].definition.value).toEqual({
      command: 'npx',
      headers: { Authorization: 'Bearer token' },
    });
  });

  it('promotes a selected conflict variant into an editable shared server', () => {
    const definition = {
      transport: 'http' as const,
      value: { url: 'https://rovo.example/mcp' },
      representable_in_form: true,
    };
    const assignment = (executor: BaseCodingAgent) => ({
      executor,
      native_name: 'Atlassian Rovo',
      native_entry: { url: 'https://rovo.example/mcp' },
      has_credentials: false,
      representable: true,
      incompatibility_reason: null,
    });
    const conflict = {
      name: 'Atlassian Rovo',
      message: 'different definitions',
      variants: [
        {
          variant_id: 'variant-1',
          definition,
          assignments: [assignment(BaseCodingAgent.CLAUDE_CODE)],
          native_sources: [],
        },
        {
          variant_id: 'variant-2',
          definition: { ...definition, value: { url: 'https://other.test' } },
          assignments: [assignment(BaseCodingAgent.CODEX)],
          native_sources: [],
        },
      ],
    };
    const resolved = resolveConflictVariant(
      { servers: [], conflicts: [conflict] },
      conflict,
      conflict.variants[0]
    );

    expect(resolved.conflicts).toEqual([]);
    expect(resolved.servers).toEqual([
      {
        name: 'Atlassian Rovo',
        definition,
        assignments: [BaseCodingAgent.CLAUDE_CODE, BaseCodingAgent.CODEX],
      },
    ]);
  });

  it('keeps custom conflict variants unresolved', () => {
    const conflict = {
      name: 'custom',
      message: 'different definitions',
      variants: [
        {
          variant_id: 'variant-1',
          definition: {
            transport: 'unknown' as const,
            value: { custom: true },
            representable_in_form: false,
          },
          assignments: [],
          native_sources: [],
        },
      ],
    };
    const state = { servers: [], conflicts: [conflict] };
    expect(resolveConflictVariant(state, conflict, conflict.variants[0])).toBe(
      state
    );
  });

  it('does not inherit assignments incompatible with the selected variant', () => {
    const assignment = (executor: BaseCodingAgent) => ({
      executor,
      native_name: 'mixed',
      native_entry: {},
      has_credentials: false,
      representable: true,
      incompatibility_reason: null,
    });
    const conflict = {
      name: 'mixed',
      message: 'different definitions',
      variants: [
        {
          variant_id: 'sse',
          definition: {
            transport: 'sse' as const,
            value: { url: 'https://example.test/sse' },
            representable_in_form: true,
          },
          assignments: [assignment(BaseCodingAgent.CLAUDE_CODE)],
          native_sources: [],
        },
        {
          variant_id: 'http',
          definition: {
            transport: 'http' as const,
            value: { url: 'https://example.test/mcp' },
            representable_in_form: true,
          },
          assignments: [
            assignment(BaseCodingAgent.CODEX),
            assignment(BaseCodingAgent.GROK),
          ],
          native_sources: [],
        },
      ],
    };

    const resolved = resolveConflictVariant(
      { servers: [], conflicts: [conflict] },
      conflict,
      conflict.variants[0]
    );

    expect(resolved.servers[0].assignments).toEqual([
      BaseCodingAgent.CLAUDE_CODE,
    ]);
  });

  it('removes the original native name when a resolved conflict is renamed', () => {
    const read = readResponse();
    read.servers = [];
    read.conflicts = [
      {
        name: 'Atlassian Rovo',
        message: 'different definitions',
        variants: [],
      },
    ];

    expect(
      removedServerNames(read, {
        servers: [
          {
            name: 'Rovo',
            definition: {
              transport: 'http',
              value: { url: 'https://rovo.example/mcp' },
              representable_in_form: true,
            },
            assignments: [BaseCodingAgent.CODEX],
          },
        ],
        conflicts: [],
      })
    ).toEqual(['Atlassian Rovo']);
  });
});
