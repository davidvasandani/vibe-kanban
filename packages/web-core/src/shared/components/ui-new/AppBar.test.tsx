/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppBar } from '@vibe/ui/components/AppBar';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

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

function renderAppBar(
  overrides: Partial<React.ComponentProps<typeof AppBar>> = {}
) {
  act(() => {
    root.render(
      <AppBar
        projects={[]}
        onCreateProject={vi.fn()}
        onWorkspacesClick={vi.fn()}
        onProjectClick={vi.fn()}
        onProjectsDragEnd={vi.fn()}
        isWorkspacesActive
        activeProjectId={null}
        userPopover={<div>Headshot</div>}
        githubIconPath=""
        discordIconPath=""
        {...overrides}
      />
    );
  });
}

describe('AppBar deployment controls', () => {
  it('renders neither revision nor web refresh below the user popover', () => {
    renderAppBar({
      appVersion: 'abc1234',
      deployUpdateAvailable: true,
      onReloadClick: vi.fn(),
    });

    expect(container.textContent).toContain('Headshot');
    expect(container.textContent).not.toContain('abc1234');
    expect(container.textContent).not.toContain('Refresh');
  });

  it('retains native Update behavior', () => {
    const onUpdateClick = vi.fn();
    renderAppBar({ updateVersion: '1.2.3', onUpdateClick });

    const update = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === 'Update'
    );
    expect(update).toBeInstanceOf(HTMLButtonElement);

    act(() => update?.click());
    expect(onUpdateClick).toHaveBeenCalledTimes(1);
  });
});
