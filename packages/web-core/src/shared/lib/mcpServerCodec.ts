import { BaseCodingAgent } from 'shared/types';
import type { JsonValue } from 'shared/types';

/**
 * Transport kinds surfaced by the MCP server form. Agents that don't
 * distinguish `http` from `sse` simply omit `sse` from their supported list.
 */
export type McpTransport = 'stdio' | 'http' | 'sse';

export interface KeyValue {
  key: string;
  value: string;
}

export interface McpServerFormValues {
  transport: McpTransport;
  // stdio
  command: string;
  args: string[];
  env: KeyValue[];
  // http / sse
  url: string;
  headers: KeyValue[];
}

export interface McpServerCodec {
  /** Transports this agent supports, in display order. */
  transports: McpTransport[];
  /**
   * Parse a native server entry into form values. Returns `null` when the
   * entry can't be faithfully represented by the form (→ treated as "custom",
   * edited as raw JSON), so the form never silently drops fields.
   */
  parse(entry: JsonValue): McpServerFormValues | null;
  /**
   * Inverse of {@link parse}. `original` (when editing an existing entry)
   * supplies keys the form doesn't model so they round-trip untouched.
   */
  serialize(values: McpServerFormValues, original?: JsonValue): JsonValue;
  /** One-line human summary for the server list card. */
  summarize(entry: JsonValue): string;
}

// --- shared helpers ---------------------------------------------------------

type JsonObject = { [key: string]: JsonValue };

function isJsonObject(v: JsonValue | undefined): v is JsonObject {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function isStringArray(v: JsonValue | undefined): v is string[] {
  return Array.isArray(v) && v.every((x) => typeof x === 'string');
}

/** True when `v` is an object whose values are all strings (env / headers). */
function isStringRecord(v: JsonValue | undefined): v is Record<string, string> {
  return (
    isJsonObject(v) && Object.values(v).every((x) => typeof x === 'string')
  );
}

function recordToPairs(v: JsonValue | undefined): KeyValue[] {
  if (!isJsonObject(v)) return [];
  return Object.entries(v).map(([key, value]) => ({
    key,
    value: typeof value === 'string' ? value : String(value ?? ''),
  }));
}

/** Convert edited key/value rows to a JSON object, dropping blank keys. */
export function pairsToRecord(pairs: KeyValue[]): JsonObject {
  const out: JsonObject = {};
  for (const { key, value } of pairs) {
    const k = key.trim();
    if (k) out[k] = value;
  }
  return out;
}

/** Split a multi-line arguments textarea into individual args. */
export function argsFromLines(text: string): string[] {
  return text
    .split('\n')
    .map((l) => l.trimEnd())
    .filter((l) => l.length > 0);
}

/**
 * True when every arg survives a line-based textarea round-trip. Empty args,
 * trailing whitespace, and embedded newlines would be silently altered by
 * {@link argsFromLines}, so such entries are treated as unrepresentable
 * (→ edited as raw JSON) rather than saved lossily.
 */
export function argsRoundTrip(args: string[]): boolean {
  return args.every(
    (a) =>
      a.length > 0 &&
      a === a.trimEnd() &&
      !a.includes('\n') &&
      !a.includes('\r')
  );
}

function emptyForm(transport: McpTransport): McpServerFormValues {
  return {
    transport,
    command: '',
    args: [],
    env: [],
    url: '',
    headers: [],
  };
}

/**
 * Preserve keys from the original entry that aren't owned by the form for the
 * given transport, so unknown/agent-specific fields survive a round-trip.
 */
function preserveExtraKeys(
  target: JsonObject,
  original: JsonValue | undefined,
  ownedKeys: string[]
): void {
  if (!isJsonObject(original)) return;
  for (const [k, v] of Object.entries(original)) {
    if (!ownedKeys.includes(k) && !(k in target)) {
      target[k] = v;
    }
  }
}

// --- claude-style (Claude Code, Amp, Droid, Copilot) ------------------------

// Union of every key this codec owns across all transports. Passing the full
// union to preserveExtraKeys ensures switching transports drops keys owned by
// the *other* transport (e.g. stale `command`/`args` when moving stdio→http)
// instead of preserving them and producing a mixed, invalid entry.
const CLAUDE_KEYS = ['type', 'command', 'args', 'env', 'url', 'headers'];

interface ClaudeStyleOptions {
  transports: McpTransport[];
}

function claudeStyleCodec({ transports }: ClaudeStyleOptions): McpServerCodec {
  const parse = (entry: JsonValue): McpServerFormValues | null => {
    if (!isJsonObject(entry)) return null;
    const type = entry.type;

    // Remote when an explicit http/sse type is present. Reject entries that
    // also carry stdio keys — such mixed shapes can't be shown losslessly, so
    // they fall through to the custom (raw-JSON) editor instead.
    if (type === 'http' || type === 'sse') {
      if (typeof entry.url !== 'string') return null;
      if (entry.command !== undefined || entry.args !== undefined) return null;
      if (entry.env !== undefined) return null;
      if (entry.headers !== undefined && !isStringRecord(entry.headers))
        return null;
      const form = emptyForm(type);
      form.url = entry.url;
      form.headers = recordToPairs(entry.headers);
      return form;
    }

    // stdio when a command is present and no remote type.
    if (type === undefined && typeof entry.command === 'string') {
      if (entry.command.length === 0) return null;
      if (entry.url !== undefined || entry.headers !== undefined) return null;
      if (entry.args !== undefined && !isStringArray(entry.args)) return null;
      if (entry.args !== undefined && !argsRoundTrip(entry.args)) return null;
      if (entry.env !== undefined && !isStringRecord(entry.env)) return null;
      const form = emptyForm('stdio');
      form.command = entry.command;
      form.args = entry.args ?? [];
      form.env = recordToPairs(entry.env);
      return form;
    }

    return null;
  };

  const serialize = (
    values: McpServerFormValues,
    original?: JsonValue
  ): JsonValue => {
    if (values.transport === 'stdio') {
      const out: JsonObject = { command: values.command };
      if (values.args.length > 0) out.args = values.args;
      const env = pairsToRecord(values.env);
      if (Object.keys(env).length > 0) out.env = env;
      preserveExtraKeys(out, original, CLAUDE_KEYS);
      return out;
    }
    const out: JsonObject = { type: values.transport, url: values.url };
    const headers = pairsToRecord(values.headers);
    if (Object.keys(headers).length > 0) out.headers = headers;
    preserveExtraKeys(out, original, CLAUDE_KEYS);
    return out;
  };

  return { transports, parse, serialize, summarize: defaultSummarize };
}

// --- cursor (remote has url but no `type`) ----------------------------------

const CURSOR_KEYS = ['command', 'args', 'env', 'url', 'headers'];

const cursorCodec: McpServerCodec = {
  transports: ['stdio', 'http'],
  parse(entry) {
    if (!isJsonObject(entry)) return null;
    if (typeof entry.command === 'string' && entry.url === undefined) {
      if (entry.command.length === 0) return null;
      if (entry.headers !== undefined) return null;
      if (entry.args !== undefined && !isStringArray(entry.args)) return null;
      if (entry.args !== undefined && !argsRoundTrip(entry.args)) return null;
      if (entry.env !== undefined && !isStringRecord(entry.env)) return null;
      const form = emptyForm('stdio');
      form.command = entry.command;
      form.args = entry.args ?? [];
      form.env = recordToPairs(entry.env);
      return form;
    }
    if (typeof entry.url === 'string' && entry.command === undefined) {
      if (entry.args !== undefined || entry.env !== undefined) return null;
      if (entry.headers !== undefined && !isStringRecord(entry.headers))
        return null;
      const form = emptyForm('http');
      form.url = entry.url;
      form.headers = recordToPairs(entry.headers);
      return form;
    }
    return null;
  },
  serialize(values, original) {
    if (values.transport === 'stdio') {
      const out: JsonObject = { command: values.command };
      if (values.args.length > 0) out.args = values.args;
      const env = pairsToRecord(values.env);
      if (Object.keys(env).length > 0) out.env = env;
      preserveExtraKeys(out, original, CURSOR_KEYS);
      return out;
    }
    const out: JsonObject = { url: values.url };
    const headers = pairsToRecord(values.headers);
    if (Object.keys(headers).length > 0) out.headers = headers;
    preserveExtraKeys(out, original, CURSOR_KEYS);
    return out;
  },
  summarize: defaultSummarize,
};

// --- gemini / qwen (remote uses httpUrl) ------------------------------------

const GEMINI_KEYS = ['command', 'args', 'env', 'httpUrl', 'headers'];

const geminiCodec: McpServerCodec = {
  transports: ['stdio', 'http'],
  parse(entry) {
    if (!isJsonObject(entry)) return null;
    if (typeof entry.command === 'string' && entry.httpUrl === undefined) {
      if (entry.command.length === 0) return null;
      if (entry.headers !== undefined) return null;
      if (entry.args !== undefined && !isStringArray(entry.args)) return null;
      if (entry.args !== undefined && !argsRoundTrip(entry.args)) return null;
      if (entry.env !== undefined && !isStringRecord(entry.env)) return null;
      const form = emptyForm('stdio');
      form.command = entry.command;
      form.args = entry.args ?? [];
      form.env = recordToPairs(entry.env);
      return form;
    }
    if (typeof entry.httpUrl === 'string' && entry.command === undefined) {
      if (entry.args !== undefined || entry.env !== undefined) return null;
      if (entry.headers !== undefined && !isStringRecord(entry.headers))
        return null;
      const form = emptyForm('http');
      form.url = entry.httpUrl;
      form.headers = recordToPairs(entry.headers);
      return form;
    }
    return null;
  },
  serialize(values, original) {
    if (values.transport === 'stdio') {
      const out: JsonObject = { command: values.command };
      if (values.args.length > 0) out.args = values.args;
      const env = pairsToRecord(values.env);
      if (Object.keys(env).length > 0) out.env = env;
      preserveExtraKeys(out, original, GEMINI_KEYS);
      return out;
    }
    const out: JsonObject = { httpUrl: values.url };
    const headers = pairsToRecord(values.headers);
    if (Object.keys(headers).length > 0) out.headers = headers;
    preserveExtraKeys(out, original, GEMINI_KEYS);
    return out;
  },
  summarize(entry) {
    if (isJsonObject(entry) && typeof entry.httpUrl === 'string')
      return entry.httpUrl;
    return defaultSummarize(entry);
  },
};

// --- codex (stdio only) -----------------------------------------------------

const codexCodec: McpServerCodec = {
  transports: ['stdio'],
  parse(entry) {
    if (!isJsonObject(entry)) return null;
    if (typeof entry.command !== 'string' || entry.command.length === 0)
      return null;
    if (entry.url !== undefined || entry.httpUrl !== undefined) return null;
    if (entry.args !== undefined && !isStringArray(entry.args)) return null;
    if (entry.args !== undefined && !argsRoundTrip(entry.args)) return null;
    if (entry.env !== undefined && !isStringRecord(entry.env)) return null;
    const form = emptyForm('stdio');
    form.command = entry.command;
    form.args = entry.args ?? [];
    form.env = recordToPairs(entry.env);
    return form;
  },
  serialize(values, original) {
    const out: JsonObject = { command: values.command };
    if (values.args.length > 0) out.args = values.args;
    const env = pairsToRecord(values.env);
    if (Object.keys(env).length > 0) out.env = env;
    preserveExtraKeys(out, original, ['command', 'args', 'env']);
    return out;
  },
  summarize: defaultSummarize,
};

// --- opencode (local/remote types, command array) ---------------------------

const OPENCODE_KEYS = [
  'type',
  'command',
  'enabled',
  'environment',
  'url',
  'headers',
];

const opencodeCodec: McpServerCodec = {
  transports: ['stdio', 'http'],
  parse(entry) {
    if (!isJsonObject(entry)) return null;
    if (entry.type === 'local') {
      if (!isStringArray(entry.command) || entry.command.length === 0)
        return null;
      if (entry.environment !== undefined && !isStringRecord(entry.environment))
        return null;
      const [command, ...args] = entry.command;
      if (command.length === 0 || !argsRoundTrip(args)) return null;
      const form = emptyForm('stdio');
      form.command = command;
      form.args = args;
      form.env = recordToPairs(entry.environment);
      return form;
    }
    if (entry.type === 'remote') {
      if (typeof entry.url !== 'string') return null;
      if (entry.headers !== undefined && !isStringRecord(entry.headers))
        return null;
      const form = emptyForm('http');
      form.url = entry.url;
      form.headers = recordToPairs(entry.headers);
      return form;
    }
    return null;
  },
  serialize(values, original) {
    if (values.transport === 'stdio') {
      const out: JsonObject = {
        type: 'local',
        command: [values.command, ...values.args],
        enabled: true,
      };
      const env = pairsToRecord(values.env);
      if (Object.keys(env).length > 0) out.environment = env;
      // Preserve a prior explicit `enabled` value.
      if (isJsonObject(original) && typeof original.enabled === 'boolean')
        out.enabled = original.enabled;
      preserveExtraKeys(out, original, OPENCODE_KEYS);
      return out;
    }
    const out: JsonObject = {
      type: 'remote',
      url: values.url,
      enabled: true,
    };
    const headers = pairsToRecord(values.headers);
    if (Object.keys(headers).length > 0) out.headers = headers;
    if (isJsonObject(original) && typeof original.enabled === 'boolean')
      out.enabled = original.enabled;
    preserveExtraKeys(out, original, OPENCODE_KEYS);
    return out;
  },
  summarize(entry) {
    if (isJsonObject(entry)) {
      if (entry.type === 'remote' && typeof entry.url === 'string')
        return entry.url;
      if (entry.type === 'local' && isStringArray(entry.command))
        return entry.command.join(' ');
    }
    return defaultSummarize(entry);
  },
};

// --- default summary --------------------------------------------------------

function defaultSummarize(entry: JsonValue): string {
  if (!isJsonObject(entry)) return '';
  if (typeof entry.url === 'string') return entry.url;
  if (typeof entry.httpUrl === 'string') return entry.httpUrl;
  if (typeof entry.command === 'string') {
    const args = isStringArray(entry.args) ? entry.args : [];
    return [entry.command, ...args].join(' ');
  }
  if (isStringArray(entry.command)) return entry.command.join(' ');
  return '';
}

// --- registry ---------------------------------------------------------------

export function codecForAgent(agent: BaseCodingAgent): McpServerCodec {
  switch (agent) {
    case BaseCodingAgent.CODEX:
      return codexCodec;
    case BaseCodingAgent.OPENCODE:
      return opencodeCodec;
    case BaseCodingAgent.GEMINI:
    case BaseCodingAgent.QWEN_CODE:
      return geminiCodec;
    case BaseCodingAgent.CURSOR_AGENT:
      return cursorCodec;
    case BaseCodingAgent.COPILOT:
    case BaseCodingAgent.CLAUDE_CODE:
    case BaseCodingAgent.AMP:
    case BaseCodingAgent.DROID:
      return claudeStyleCodec({ transports: ['stdio', 'http', 'sse'] });
    case BaseCodingAgent.GROK:
      return claudeStyleCodec({ transports: ['stdio', 'http'] });
    default:
      return claudeStyleCodec({ transports: ['stdio', 'http', 'sse'] });
  }
}

/** Transport shown on the list card badge; `null` for custom (unparseable). */
export function transportOf(
  codec: McpServerCodec,
  entry: JsonValue
): McpTransport | null {
  return codec.parse(entry)?.transport ?? null;
}
