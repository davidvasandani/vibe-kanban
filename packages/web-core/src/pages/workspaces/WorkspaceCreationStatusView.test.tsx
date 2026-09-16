/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Workspace, WorkspaceCreationProgress } from 'shared/types';
import { WorkspaceCreationStatusView } from './WorkspaceCreationStatusView';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const query = vi.hoisted(() => ({
  data: undefined as WorkspaceCreationProgress | undefined,
  isError: false,
}));
vi.mock('@/shared/hooks/useWorkspaceCreationProgress', () => ({
  useWorkspaceCreationProgress: () => query,
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  query.data = undefined;
  query.isError = false;
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

function progress(status: WorkspaceCreationProgress['status'] = 'running') {
  query.data = {
    workspace_id: 'workspace-id',
    status,
    phase: 'worktrees',
    updated_at: '2026-09-16T10:00:00Z',
  };
}

describe('WorkspaceCreationStatusView', () => {
  it('shows real ordered progress and the current background task', () => {
    progress();
    renderStatus(workspace('running'));
    const steps = [...container.querySelectorAll('li')];
    expect(steps).toHaveLength(6);
    expect(steps[0].textContent).toContain('workspaceCreation.states.complete');
    expect(steps[3].textContent).toContain('workspaceCreation.states.running');
    expect(steps[4].textContent).toContain('workspaceCreation.states.pending');
    expect(container.textContent).toContain('workspaceCreation.backgroundTask');
    expect(container.textContent).toContain('workspaceCreation.lastReported');
  });

  it('keeps queued phases pending', () => {
    renderStatus(workspace('queued'));
    expect(container.textContent).toContain('workspaceCreation.queued');
    expect(container.querySelectorAll('li')).toHaveLength(6);
    expect(container.textContent).not.toContain(
      'workspaceCreation.states.complete'
    );
    expect(container.textContent).not.toContain(
      'workspaceCreation.states.running'
    );
  });

  it.each(['prop', 'snapshot'] as const)(
    'stops active indicators on failure from %s',
    (source) => {
      progress(source === 'snapshot' ? 'failed' : 'running');
      renderStatus(workspace(source === 'prop' ? 'failed' : 'running'));
      expect(container.querySelector('[role="alert"]')).not.toBeNull();
      expect(container.querySelectorAll('li')[3].textContent).toContain(
        'workspaceCreation.states.failed'
      );
      expect(container.textContent).not.toContain(
        'workspaceCreation.states.running'
      );
      expect(container.querySelector('.animate-pulse')).toBeNull();
    }
  );

  it('shows unavailable reporting without asserting ongoing activity', () => {
    progress();
    query.isError = true;
    renderStatus(workspace('running'));
    expect(container.textContent).toContain('workspaceCreation.unavailable');
    expect(container.textContent).toContain(
      'workspaceCreation.states.lastReported'
    );
    expect(container.querySelector('.animate-pulse')).toBeNull();
  });

  it('does not claim unreported work never started', () => {
    renderStatus(workspace('failed'));
    expect(container.textContent).toContain('workspaceCreation.states.unknown');
    expect(container.textContent).not.toContain(
      'workspaceCreation.states.pending'
    );
    renderStatus(workspace('running'));
    expect(container.textContent).toContain('workspaceCreation.states.unknown');
  });

  it('never shows another workspace phase', () => {
    progress();
    query.data!.workspace_id = 'another-workspace';
    renderStatus(workspace('running'));
    expect(container.textContent).toContain('workspaceCreation.loading');
    expect(container.textContent).not.toContain(
      'workspaceCreation.states.complete'
    );
  });

  it('stops indicating activity when completion arrives before workspace refresh', () => {
    progress('ready');
    renderStatus(workspace('running'));
    expect(container.textContent).toContain('workspaceCreation.opening');
    expect(container.querySelector('.animate-pulse')).toBeNull();
  });

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
