/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { expect, it } from 'vitest';
import { ArchiveProvenanceDetails } from './ArchiveProvenance';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
it('shows the archive rule, time, acknowledgement and recovery advice', () => {
  const container = document.createElement('div');
  const root = createRoot(container);
  act(() =>
    root.render(
      <ArchiveProvenanceDetails
        archived
        events={[
          {
            workspace_id: 'w',
            trigger_key: 'event',
            rule: 'issue_done',
            triggered_at: '2026-10-25T15:13:06Z',
            archived_at: '2026-10-25T15:13:06Z',
            acknowledged: true,
          },
        ]}
      />
    )
  );
  expect(container.textContent).toContain('Archived by issue_done');
  expect(container.querySelector('time')?.dateTime).toBe(
    '2026-10-25T15:13:06Z'
  );
  expect(container.textContent).toContain('Explicitly unarchived');
  expect(container.textContent).toContain('Scheduling a wake-up');
  act(() => root.render(<ArchiveProvenanceDetails archived events={[]} />));
  expect(container.textContent).toContain('rule is unknown');
  act(() => root.unmount());
});

it('shows a new pending completion before acknowledged archive history', () => {
  const container = document.createElement('div');
  const root = createRoot(container);
  const common = { workspace_id: 'w', triggered_at: '2026-10-25T15:13:06Z' };
  act(() =>
    root.render(
      <ArchiveProvenanceDetails
        archived={false}
        events={[
          {
            ...common,
            trigger_key: 'old',
            rule: 'pr_merged',
            archived_at: common.triggered_at,
            acknowledged: true,
          },
          {
            ...common,
            trigger_key: 'new',
            rule: 'issue_done',
            archived_at: null,
            acknowledged: false,
          },
        ]}
      />
    )
  );
  expect(container.textContent).toContain('Archive requested by issue_done');
  expect(container.textContent).not.toContain('Explicitly unarchived');
  act(() => root.unmount());
});

it('attributes an archived workspace to the event actually admitted', () => {
  const container = document.createElement('div');
  const root = createRoot(container);
  const common = {
    workspace_id: 'w',
    triggered_at: '2026-10-25T15:13:06Z',
    acknowledged: false,
  };
  act(() =>
    root.render(
      <ArchiveProvenanceDetails
        archived
        events={[
          {
            ...common,
            trigger_key: 'remote',
            rule: 'issue_done',
            archived_at: null,
          },
          {
            ...common,
            trigger_key: 'local',
            rule: 'pr_merged',
            archived_at: common.triggered_at,
          },
        ]}
      />
    )
  );
  expect(container.textContent).toContain('Archived by pr_merged');
  expect(container.textContent).not.toContain('issue_done');
  act(() => root.unmount());
});
