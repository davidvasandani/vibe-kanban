import { describe, expect, it } from 'vitest';
import { BaseCodingAgent } from 'shared/types';
import type { SharedMcpReadResponse } from 'shared/types';
import {
  definitionFromEntry,
  draftFromSharedRead,
  indexAssignmentTests,
  inputsFromDraft,
  mergeOAuthRefresh,
  nextAvailableServerName,
  preconfiguredMcpServers,
  removedServerNames,
  resolveConflictVariant,
  sharedMcpSnapshot,
  takenServerNames,
  testKey,
  testTargetsForDraft,
} from './sharedMcpSettingsState';
import type { SharedMcpDraftState } from './sharedMcpSettingsState';

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

  describe('nextAvailableServerName', () => {
    it('uses the catalog key when nothing has claimed it', () => {
      expect(nextAvailableServerName('gmail', [])).toBe('gmail');
      expect(nextAvailableServerName('gmail', ['slack', 'context7'])).toBe(
        'gmail'
      );
    });

    it('suffixes later instances of the same template', () => {
      expect(nextAvailableServerName('gmail', ['gmail'])).toBe('gmail_2');
      expect(nextAvailableServerName('gmail', ['gmail', 'gmail_2'])).toBe(
        'gmail_3'
      );
    });

    it('fills a gap rather than counting instances', () => {
      // A user who added three and deleted the second should get `gmail_2`
      // back, not `gmail_4` — the result depends only on what is taken.
      expect(nextAvailableServerName('gmail', ['gmail', 'gmail_3'])).toBe(
        'gmail_2'
      );
    });

    it('never returns a name that is already taken', () => {
      // Reusing a name is worse than an error: `setServer` de-duplicates by
      // name, so the new server would replace the existing one silently.
      const existing = ['gmail', 'gmail_2', 'gmail_3', 'gmail_4'];
      expect(existing).not.toContain(
        nextAvailableServerName('gmail', existing)
      );
    });

    it('generates identifiers the backend will accept', () => {
      // Bound to `is_valid_server_identifier` (^[a-zA-Z0-9_-]+$) in
      // crates/executors/src/shared_mcp_config.rs. A generated name that fails
      // this is rejected on save, or silently rewritten by
      // `suggested_server_identifier`.
      const keys = ['gmail', 'slack', 'chrome_devtools', 'dev-manager'];
      const taken: string[] = [];
      for (const key of keys) {
        for (let i = 0; i < 5; i += 1) {
          const name = nextAvailableServerName(key, taken);
          expect(name).toMatch(/^[a-zA-Z0-9_-]+$/);
          taken.push(name);
        }
      }
    });

    it('treats a conflicting name as taken', () => {
      // A name whose definitions diverge across agents lives in `conflicts`,
      // not `servers`. Allocating `gmail_2` while an unresolved `gmail_2`
      // conflict exists would bind the new definition to that conflict and
      // drop the native entry it was still arbitrating.
      const draft = {
        servers: [
          {
            name: 'gmail',
            definition: {
              transport: 'stdio' as const,
              value: { command: 'npx' },
              representable_in_form: true,
            },
            assignments: [BaseCodingAgent.CLAUDE_CODE],
          },
        ],
        conflicts: [
          { name: 'gmail_2', reason: 'differs', variants: [] },
        ] as unknown as SharedMcpDraftState['conflicts'],
      };
      expect(takenServerNames(draft)).toEqual(['gmail', 'gmail_2']);
      expect(nextAvailableServerName('gmail', takenServerNames(draft))).toBe(
        'gmail_3'
      );
    });

    it('yields a distinct server each time a template is added repeatedly', () => {
      const names: string[] = [];
      for (let i = 0; i < 3; i += 1) {
        names.push(nextAvailableServerName('gmail', names));
      }
      expect(names).toEqual(['gmail', 'gmail_2', 'gmail_3']);
      expect(new Set(names).size).toBe(3);
    });
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
