import { describe, expect, it } from 'vitest';
import { BaseCodingAgent } from 'shared/types';
import type { JsonValue } from 'shared/types';
import {
  argsFromLines,
  codecForAgent,
  pairsToRecord,
  transportOf,
} from './mcpServerCodec';

/** parse → serialize on an untouched form must round-trip the entry exactly. */
function expectRoundTrip(agent: BaseCodingAgent, entry: JsonValue) {
  const codec = codecForAgent(agent);
  const form = codec.parse(entry);
  expect(form).not.toBeNull();
  expect(codec.serialize(form!, entry)).toEqual(entry);
}

describe('claude-style codec (Claude Code)', () => {
  it('round-trips a stdio server', () => {
    expectRoundTrip(BaseCodingAgent.CLAUDE_CODE, {
      command: 'npx',
      args: ['-y', 'vibe-kanban@latest', '--mcp'],
    });
  });

  it('round-trips an http server with headers', () => {
    expectRoundTrip(BaseCodingAgent.CLAUDE_CODE, {
      type: 'http',
      url: 'https://mcp.example.com/mcp',
      headers: { Authorization: 'Bearer x' },
    });
  });

  it('round-trips an sse server', () => {
    expectRoundTrip(BaseCodingAgent.CLAUDE_CODE, {
      type: 'sse',
      url: 'http://127.0.0.1:3334/sse',
    });
  });

  it('preserves unknown keys (env + extras) through an edit', () => {
    const entry: JsonValue = {
      command: 'my-server',
      args: ['run'],
      env: { API_KEY: 'secret' },
      cwd: '/tmp',
    };
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    const form = codec.parse(entry)!;
    form.args = ['run', '--verbose'];
    const out = codec.serialize(form, entry) as Record<string, JsonValue>;
    expect(out.args).toEqual(['run', '--verbose']);
    expect(out.env).toEqual({ API_KEY: 'secret' });
    expect(out.cwd).toBe('/tmp');
  });

  it('rejects mixed stdio+remote entries as custom (null)', () => {
    // A shape that mixes transports can't be shown losslessly, so it falls
    // through to the raw-JSON editor rather than being silently normalized.
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    expect(codec.parse({ type: 'http', url: 'y', command: 'x' })).toBeNull();
    expect(codec.parse({ command: 'x', url: 'y' })).toBeNull();
  });

  it('returns null for non-string command', () => {
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    expect(codec.parse({ command: 123 } as unknown as JsonValue)).toBeNull();
  });

  it('drops stale stdio keys when switching to http (with original)', () => {
    // Regression: switching transport must not carry over the previous
    // transport's keys from the original entry.
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    const original: JsonValue = {
      command: 'x',
      args: ['a'],
      env: { K: 'v' },
    };
    const form = codec.parse(original)!;
    form.transport = 'http';
    form.url = 'https://e.com';
    const out = codec.serialize(form, original) as Record<string, JsonValue>;
    expect(out).toEqual({ type: 'http', url: 'https://e.com' });
    expect(out.command).toBeUndefined();
    expect(out.args).toBeUndefined();
    expect(out.env).toBeUndefined();
  });

  it('drops stale remote keys when switching http to stdio (with original)', () => {
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    const original: JsonValue = {
      type: 'http',
      url: 'https://e.com',
      headers: { A: 'b' },
    };
    const form = codec.parse(original)!;
    form.transport = 'stdio';
    form.command = 'npx';
    const out = codec.serialize(form, original) as Record<string, JsonValue>;
    expect(out).toEqual({ command: 'npx' });
  });
});

describe('args representability', () => {
  it('rejects args with empty strings (would be dropped on save)', () => {
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    expect(codec.parse({ command: 'x', args: ['-y', '', 'srv'] })).toBeNull();
  });

  it('rejects args with trailing whitespace', () => {
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    expect(codec.parse({ command: 'x', args: ['value '] })).toBeNull();
  });

  it('rejects args containing newlines', () => {
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    expect(codec.parse({ command: 'x', args: ['a\nb'] })).toBeNull();
  });

  it('accepts args with leading whitespace (survives round-trip)', () => {
    expectRoundTrip(BaseCodingAgent.CLAUDE_CODE, {
      command: 'x',
      args: ['  leading'],
    });
  });

  it('rejects opencode local command whose tail args are unrepresentable', () => {
    const codec = codecForAgent(BaseCodingAgent.OPENCODE);
    expect(
      codec.parse({ type: 'local', command: ['npx', ''], enabled: true })
    ).toBeNull();
  });
});

describe('copilot codec preserves tools', () => {
  it('keeps the tools array', () => {
    const entry: JsonValue = {
      command: 'srv',
      tools: ['*'],
    };
    const codec = codecForAgent(BaseCodingAgent.COPILOT);
    const form = codec.parse(entry)!;
    const out = codec.serialize(form, entry) as Record<string, JsonValue>;
    expect(out.tools).toEqual(['*']);
  });
});

describe('cursor codec (typeless remote url)', () => {
  it('round-trips a url server without a type field', () => {
    expectRoundTrip(BaseCodingAgent.CURSOR_AGENT, {
      url: 'https://mcp.example.com/mcp',
    });
  });

  it('serializes remote without a type key', () => {
    const codec = codecForAgent(BaseCodingAgent.CURSOR_AGENT);
    const form = codec.parse({ url: 'https://e.com' })!;
    const out = codec.serialize(form) as Record<string, JsonValue>;
    expect(out.type).toBeUndefined();
    expect(out.url).toBe('https://e.com');
  });

  it('offers only stdio + http transports', () => {
    expect(codecForAgent(BaseCodingAgent.CURSOR_AGENT).transports).toEqual([
      'stdio',
      'http',
    ]);
  });
});

describe('grok codec (native TOML shape)', () => {
  it('round-trips stdio and typeless http entries', () => {
    expectRoundTrip(BaseCodingAgent.GROK, {
      command: 'npx',
      args: ['-y', 'server'],
      env: { TOKEN: '${TOKEN}' },
    });
    expectRoundTrip(BaseCodingAgent.GROK, {
      url: 'https://mcp.example.com/mcp',
      headers: { Authorization: 'Bearer ${TOKEN}' },
    });
  });

  it('offers only the transports documented by Grok Build', () => {
    expect(codecForAgent(BaseCodingAgent.GROK).transports).toEqual([
      'stdio',
      'http',
    ]);
  });
});

describe('gemini codec (httpUrl)', () => {
  it('round-trips an httpUrl server', () => {
    expectRoundTrip(BaseCodingAgent.GEMINI, {
      httpUrl: 'https://mcp.example.com/mcp',
      headers: { Accept: 'application/json' },
    });
  });

  it('summarizes using httpUrl', () => {
    const codec = codecForAgent(BaseCodingAgent.GEMINI);
    expect(codec.summarize({ httpUrl: 'https://e.com' })).toBe('https://e.com');
  });

  it('qwen uses the same codec', () => {
    expectRoundTrip(BaseCodingAgent.QWEN_CODE, {
      httpUrl: 'https://mcp.example.com/mcp',
    });
  });
});

describe('codex codec (stdio only)', () => {
  it('only supports stdio', () => {
    expect(codecForAgent(BaseCodingAgent.CODEX).transports).toEqual(['stdio']);
  });

  it('round-trips a stdio server', () => {
    expectRoundTrip(BaseCodingAgent.CODEX, {
      command: 'npx',
      args: ['-y', 'server'],
      env: { KEY: 'v' },
    });
  });

  it('rejects remote entries', () => {
    const codec = codecForAgent(BaseCodingAgent.CODEX);
    expect(codec.parse({ command: 'x', url: 'y' })).toBeNull();
  });
});

describe('opencode codec (local/remote, command array)', () => {
  it('round-trips a local server', () => {
    expectRoundTrip(BaseCodingAgent.OPENCODE, {
      type: 'local',
      command: ['npx', '-y', 'server', '--mcp'],
      enabled: true,
    });
  });

  it('round-trips a remote server', () => {
    expectRoundTrip(BaseCodingAgent.OPENCODE, {
      type: 'remote',
      url: 'https://mcp.example.com/mcp',
      enabled: true,
      headers: { Accept: 'application/json, text/event-stream' },
    });
  });

  it('splits command array into command + args', () => {
    const codec = codecForAgent(BaseCodingAgent.OPENCODE);
    const form = codec.parse({
      type: 'local',
      command: ['npx', '-y', 'server'],
      enabled: true,
    })!;
    expect(form.command).toBe('npx');
    expect(form.args).toEqual(['-y', 'server']);
  });

  it('defaults enabled:true for a new local server', () => {
    const codec = codecForAgent(BaseCodingAgent.OPENCODE);
    const out = codec.serialize({
      transport: 'stdio',
      command: 'npx',
      args: ['server'],
      env: [],
      url: '',
      headers: [],
    }) as Record<string, JsonValue>;
    expect(out).toEqual({
      type: 'local',
      command: ['npx', 'server'],
      enabled: true,
    });
  });

  it('preserves an explicit enabled:false', () => {
    const entry: JsonValue = {
      type: 'local',
      command: ['npx', 'server'],
      enabled: false,
    };
    const codec = codecForAgent(BaseCodingAgent.OPENCODE);
    const form = codec.parse(entry)!;
    const out = codec.serialize(form, entry) as Record<string, JsonValue>;
    expect(out.enabled).toBe(false);
  });
});

describe('transportOf', () => {
  it('returns the parsed transport', () => {
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    expect(transportOf(codec, { command: 'x' })).toBe('stdio');
    expect(transportOf(codec, { type: 'http', url: 'y' })).toBe('http');
  });

  it('returns null for unparseable (custom) entries', () => {
    const codec = codecForAgent(BaseCodingAgent.CLAUDE_CODE);
    expect(transportOf(codec, { weird: true })).toBeNull();
  });
});

describe('helpers', () => {
  it('argsFromLines drops blank lines', () => {
    expect(argsFromLines('-y\n\nserver\n')).toEqual(['-y', 'server']);
  });

  it('pairsToRecord drops blank keys and trims', () => {
    expect(
      pairsToRecord([
        { key: ' KEY ', value: 'v' },
        { key: '', value: 'ignored' },
      ])
    ).toEqual({ KEY: 'v' });
  });
});
