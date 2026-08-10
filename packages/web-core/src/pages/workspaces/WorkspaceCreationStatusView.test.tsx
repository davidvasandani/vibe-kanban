/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Workspace } from 'shared/types';
import { WorkspaceCreationStatusView } from './WorkspaceCreationStatusView';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
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

function workspace(
  creation_status: Workspace['creation_status'],
  creation_error: string | null = null
): Workspace {
  return {
    id: 'workspace-id',
    task_id: null,
    container_ref: null,
    branch: 'vk/test',
    setup_completed_at: null,
    created_at: '2026-08-10T00:00:00Z',
    updated_at: '2026-08-10T00:00:00Z',
    archived: false,
    pinned: false,
    name: null,
    worktree_deleted: false,
    current_pipeline_stage: null,
    speckit_feature_key: null,
    speckit_host_repo_id: null,
    creation_status,
    creation_error,
  };
}

function renderStatus(value: Workspace) {
  act(() => root.render(<WorkspaceCreationStatusView workspace={value} />));
}

describe('WorkspaceCreationStatusView', () => {
  it.each(['queued', 'running'] as const)(
    'renders pending state for %s creation',
    (status) => {
      renderStatus(workspace(status));
      expect(container.querySelector('[role="status"]')?.textContent).toContain(
        'workspaceCreation.creatingTitle'
      );
    }
  );

  it('renders the persisted creation failure', () => {
    renderStatus(workspace('failed', 'Repository setup failed'));
    expect(container.querySelector('[role="alert"]')?.textContent).toContain(
      'Repository setup failed'
    );
  });

  it('renders nothing for a ready workspace', () => {
    renderStatus(workspace('ready'));
    expect(container.innerHTML).toBe('');
  });
});
