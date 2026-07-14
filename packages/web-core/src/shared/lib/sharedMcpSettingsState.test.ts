import { describe, expect, it } from 'vitest';
import { BaseCodingAgent } from 'shared/types';
import type { SharedMcpReadResponse } from 'shared/types';
import {
  definitionFromEntry,
  draftFromSharedRead,
  indexAssignmentTests,
  inputsFromDraft,
  mergeOAuthRefresh,
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
});
