/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { NormalizedEntry } from 'shared/types';

import type {
  AggregatedPatchGroup,
  PatchTypeWithKey,
} from '@/shared/hooks/useConversationHistory/types';
import {
  ConversationTimestamp,
  formatConversationTimestamp,
  getDisplayEntryTimestamp,
} from './ConversationTimestamp';

function normalizedEntry(
  timestamp: string | null,
  type: NormalizedEntry['entry_type']['type'] = 'assistant_message',
  patchKey = 'process:0',
  processCreatedAt?: string
): PatchTypeWithKey {
  return {
    type: 'NORMALIZED_ENTRY',
    content: {
      timestamp,
      content: 'Entry',
      entry_type: { type } as NormalizedEntry['entry_type'],
    },
    patchKey,
    executionProcessId: 'process',
    processCreatedAt,
  };
}

describe('conversation timestamps', () => {
  it('formats same-day and cross-day values without changing the source value', () => {
    const now = new Date('2026-09-16T18:00:00Z');
    const sameDay = formatConversationTimestamp(
      '2026-09-16T15:04:00Z',
      now,
      'en-US'
    );
    const otherDay = formatConversationTimestamp(
      '2026-09-15T15:04:00Z',
      now,
      'en-US'
    );

    expect(sameDay?.dateTime).toBe('2026-09-16T15:04:00Z');
    expect(sameDay?.compactLabel).not.toContain('Sep');
    expect(otherDay?.compactLabel).toContain('Sep');
    expect(otherDay?.fullLabel).toContain('2026');
  });

  it('omits absent and malformed timestamps', () => {
    expect(formatConversationTimestamp(null)).toBeNull();
    expect(formatConversationTimestamp('not-a-date')).toBeNull();
  });

  it('falls back to the authoritative process time when event time is absent', () => {
    expect(
      getDisplayEntryTimestamp(
        normalizedEntry(
          null,
          'system_message',
          'process:0',
          '2026-09-16T09:30:00Z'
        )
      )
    ).toBe('2026-09-16T09:30:00Z');
  });

  it('falls back to the process time when an event time is malformed', () => {
    expect(
      getDisplayEntryTimestamp(
        normalizedEntry(
          'not-a-date',
          'system_message',
          'process:0',
          '2026-09-16T09:30:00Z'
        )
      )
    ).toBe('2026-09-16T09:30:00Z');
  });

  it('uses the latest valid timestamp in an aggregate', () => {
    const group: AggregatedPatchGroup = {
      type: 'AGGREGATED_GROUP',
      aggregationType: 'file_read',
      entries: [
        normalizedEntry(
          '2026-09-16T10:00:00Z',
          'assistant_message',
          'process:0'
        ),
        normalizedEntry('invalid', 'assistant_message', 'process:1'),
        normalizedEntry(
          '2026-09-16T10:02:00Z',
          'assistant_message',
          'process:2'
        ),
      ],
      patchKey: 'agg:process:0',
      executionProcessId: 'process',
    };

    expect(getDisplayEntryTimestamp(group)).toBe('2026-09-16T10:02:00Z');
  });

  it('does not timestamp synthetic presentation controls', () => {
    expect(
      getDisplayEntryTimestamp(normalizedEntry(null, 'loading'))
    ).toBeNull();
    expect(
      getDisplayEntryTimestamp(
        normalizedEntry('2026-09-16T10:00:00Z', 'token_usage_info')
      )
    ).toBeNull();
  });

  it('does not leave timestamps for deliberately hidden tool entries', () => {
    const timestamp = '2026-09-16T10:00:00Z';
    const askQuestion = normalizedEntry(timestamp, 'tool_use');
    const emptyPlan = normalizedEntry(timestamp, 'tool_use');

    if (
      askQuestion.type !== 'NORMALIZED_ENTRY' ||
      emptyPlan.type !== 'NORMALIZED_ENTRY'
    ) {
      throw new Error('Expected normalized fixtures');
    }

    askQuestion.content.entry_type = {
      type: 'tool_use',
      tool_name: 'AskUserQuestion',
      action_type: {
        action: 'ask_user_question',
        questions: [],
      },
      status: { status: 'success' },
    };
    emptyPlan.content.entry_type = {
      type: 'tool_use',
      tool_name: 'Plan',
      action_type: {
        action: 'plan_presentation',
        plan: '   ',
      },
      status: { status: 'success' },
    };

    expect(getDisplayEntryTimestamp(askQuestion)).toBeNull();
    expect(getDisplayEntryTimestamp(emptyPlan)).toBeNull();
  });
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('ConversationTimestamp', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('renders semantic full-time metadata for a logged entry', () => {
    const sourceTimestamp = '2026-09-16T10:00:00Z';
    act(() =>
      root.render(
        <ConversationTimestamp entry={normalizedEntry(sourceTimestamp)} />
      )
    );

    const time = container.querySelector('time');
    expect(time?.getAttribute('datetime')).toBe(sourceTimestamp);
    expect(time?.getAttribute('title')).toBeTruthy();
    expect(time?.getAttribute('aria-label')).toBe(time?.getAttribute('title'));
  });

  it('renders nothing for invalid time', () => {
    act(() =>
      root.render(<ConversationTimestamp entry={normalizedEntry('invalid')} />)
    );
    expect(container.querySelector('time')).toBeNull();
  });
});
