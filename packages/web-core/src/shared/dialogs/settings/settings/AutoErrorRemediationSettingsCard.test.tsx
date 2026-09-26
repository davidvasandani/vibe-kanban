/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutoErrorRemediationConfig } from 'shared/types';
import { AutoErrorRemediationSettingsCard } from './AutoErrorRemediationSettingsCard';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/shared/hooks/useAllOrganizationProjects', () => ({
  useAllOrganizationProjects: () => ({ data: [], isLoading: false }),
}));
vi.mock('@tanstack/react-query', () => ({
  useQuery: () => ({
    data: [
      { id: 'repo-a', name: 'homelab', display_name: 'homelab' },
      { id: 'repo-b', name: 'vibe-kanban', display_name: 'vibe-kanban' },
    ],
  }),
}));

const baseConfig: AutoErrorRemediationConfig = {
  enabled: false,
  project_id: null,
  repo_ids: ['repo-a', 'repo-b'],
  executor: 'CLAUDE_CODE',
  variant: 'PROALIGN',
  model_id: 'claude-opus-5-5',
  pipeline_ids: ['wikillm', 'speckit'],
  merge_stage_ids: ['pr-and-merge', 'merge'],
  max_per_hour: 3,
};

let container: HTMLDivElement;
let root: Root;
const onChange = vi.fn();

function checkbox(id: string) {
  return container.querySelector<HTMLInputElement>(`#${id}`)!;
}

beforeEach(() => {
  onChange.mockReset();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  act(() =>
    root.render(
      <AutoErrorRemediationSettingsCard
        value={baseConfig}
        executors={['CLAUDE_CODE', 'CODEX']}
        onChange={onChange}
      />
    )
  );
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('AutoErrorRemediationSettingsCard', () => {
  it('toggles the feature without touching other settings', () => {
    act(() => checkbox('auto-error-remediation-enabled').click());
    expect(onChange).toHaveBeenCalledWith({ ...baseConfig, enabled: true });
  });

  it('removes a deselected repository from the whole list', () => {
    expect(checkbox('auto-error-remediation-repo-repo-a').checked).toBe(true);
    act(() => checkbox('auto-error-remediation-repo-repo-a').click());
    expect(onChange).toHaveBeenCalledWith({
      ...baseConfig,
      repo_ids: ['repo-b'],
    });
  });
});
