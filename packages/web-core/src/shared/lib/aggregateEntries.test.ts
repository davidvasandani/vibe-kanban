import { describe, expect, it } from 'vitest';
import type { ActionType, JsonValue, ToolStatus } from 'shared/types';

import type {
  AggregatedPatchGroup,
  DisplayEntry,
  PatchTypeWithKey,
} from '@/shared/hooks/useConversationHistory/types';
import {
  aggregateConsecutiveEntries,
  formatToolArgsPreview,
} from './aggregateEntries';

let keyCounter = 0;

function toolUse(
  toolName: string,
  actionType: ActionType,
  status: ToolStatus = { status: 'success' }
): PatchTypeWithKey {
  keyCounter += 1;
  return {
    type: 'NORMALIZED_ENTRY',
    content: {
      timestamp: null,
      content: toolName,
      entry_type: {
        type: 'tool_use',
        tool_name: toolName,
        action_type: actionType,
        status,
      },
    },
    patchKey: `process:${keyCounter}`,
    executionProcessId: 'process',
  };
}

function mcpCall(
  toolName: string,
  args: JsonValue | null = null,
  status?: ToolStatus
): PatchTypeWithKey {
  return toolUse(
    toolName,
    { action: 'tool', tool_name: toolName, arguments: args, result: null },
    status
  );
}

function fileRead(path: string): PatchTypeWithKey {
  return toolUse('Read', { action: 'file_read', path });
}

function assistantMessage(): PatchTypeWithKey {
  keyCounter += 1;
  return {
    type: 'NORMALIZED_ENTRY',
    content: {
      timestamp: null,
      content: 'Working on it',
      entry_type: { type: 'assistant_message' },
    },
    patchKey: `process:${keyCounter}`,
    executionProcessId: 'process',
  };
}

function asGroup(entry: DisplayEntry): AggregatedPatchGroup {
  expect(entry.type).toBe('AGGREGATED_GROUP');
  return entry as AggregatedPatchGroup;
}

const SOLINK = 'mcp:sgsc-mcp:solink';

describe('aggregateConsecutiveEntries — generic tool calls', () => {
  it('collapses consecutive calls to the same tool into one group', () => {
    const calls = Array.from({ length: 8 }, (_, i) =>
      mcpCall(SOLINK, { site: i })
    );

    const result = aggregateConsecutiveEntries(calls);

    expect(result).toHaveLength(1);
    const group = asGroup(result[0]);
    expect(group.aggregationType).toBe('tool');
    expect(group.toolName).toBe(SOLINK);
    expect(group.entries).toEqual(calls);
    expect(group.patchKey).toBe(`agg:${calls[0].patchKey}`);
  });

  it('splits runs when the tool name changes', () => {
    const a1 = mcpCall(SOLINK);
    const a2 = mcpCall(SOLINK);
    const b = mcpCall('mcp:slack:channels_list');
    const a3 = mcpCall(SOLINK);

    const result = aggregateConsecutiveEntries([a1, a2, b, a3]);

    expect(result).toHaveLength(3);
    expect(asGroup(result[0]).entries).toEqual([a1, a2]);
    expect(result[1]).toBe(b);
    expect(result[2]).toBe(a3);
  });

  it('leaves a single call ungrouped', () => {
    const call = mcpCall(SOLINK);
    expect(aggregateConsecutiveEntries([call])).toEqual([call]);
  });

  it('only groups consecutive calls', () => {
    const a1 = mcpCall(SOLINK);
    const message = assistantMessage();
    const a2 = mcpCall(SOLINK);

    expect(aggregateConsecutiveEntries([a1, message, a2])).toEqual([
      a1,
      message,
      a2,
    ]);
  });

  it('never groups calls awaiting approval and does not merge across them', () => {
    const a1 = mcpCall(SOLINK);
    const pending = mcpCall(SOLINK, null, {
      status: 'pending_approval',
      approval_id: 'approval-1',
    });
    const a2 = mcpCall(SOLINK);
    const a3 = mcpCall(SOLINK);

    const result = aggregateConsecutiveEntries([a1, pending, a2, a3]);

    expect(result).toHaveLength(3);
    expect(result[0]).toBe(a1);
    expect(result[1]).toBe(pending);
    expect(asGroup(result[2]).entries).toEqual([a2, a3]);
  });

  it('keeps existing file_read groups separate from tool groups', () => {
    const r1 = fileRead('a.ts');
    const r2 = fileRead('b.ts');
    const c1 = mcpCall(SOLINK);
    const c2 = mcpCall(SOLINK);

    const result = aggregateConsecutiveEntries([r1, r2, c1, c2]);

    expect(result).toHaveLength(2);
    const reads = asGroup(result[0]);
    expect(reads.aggregationType).toBe('file_read');
    expect(reads.toolName).toBeUndefined();
    expect(reads.entries).toEqual([r1, r2]);
    expect(asGroup(result[1]).entries).toEqual([c1, c2]);
  });
});

describe('formatToolArgsPreview', () => {
  it('returns an empty string when there is nothing to show', () => {
    expect(formatToolArgsPreview(null)).toBe('');
    expect(formatToolArgsPreview(undefined)).toBe('');
    expect(formatToolArgsPreview({})).toBe('');
    expect(formatToolArgsPreview([])).toBe('');
  });

  it('lists top-level arguments on one line', () => {
    expect(
      formatToolArgsPreview({
        site_id: 42,
        query: 'open\n  doors',
        live: true,
        cursor: null,
        cameras: [1, 2, 3],
        filter: { kind: 'motion' },
      })
    ).toBe(
      'site_id: 42, query: open doors, live: true, cursor: null, cameras: [3], filter: {…}'
    );
  });

  it('renders scalar arguments directly', () => {
    expect(formatToolArgsPreview('lookup')).toBe('lookup');
    expect(formatToolArgsPreview(7)).toBe('7');
  });

  it('truncates long previews', () => {
    const preview = formatToolArgsPreview({ text: 'x'.repeat(200) }, 20);
    expect(preview).toHaveLength(20);
    expect(preview.endsWith('…')).toBe(true);
  });
});
