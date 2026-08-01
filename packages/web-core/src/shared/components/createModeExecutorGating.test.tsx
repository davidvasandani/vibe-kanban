/* @vitest-environment jsdom */
/**
 * FR-7: the create-mode agent picker must show an agent the cluster cannot run
 * as disabled *with a visible reason*, rather than letting the user pick it and
 * discover the problem from a rejected submission.
 *
 * The pure capability logic is covered in `lib/workerCapabilities.test.ts`.
 * This exercises the rendering contract that logic feeds, because a helper
 * returning the right map proves nothing about what the user sees.
 */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CreateChatBox } from '@vibe/ui/components/CreateChatBox';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
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
  vi.clearAllMocks();
});

function renderPicker(unsupported?: ReadonlyMap<string, string>) {
  const onChange = vi.fn();
  act(() => {
    root.render(
      <CreateChatBox
        editor={{ value: '', onChange: () => {} }}
        renderEditor={() => <textarea readOnly value="" />}
        onSend={() => {}}
        isSending={false}
        executor={{
          selected: 'CLAUDE_CODE',
          options: ['CLAUDE_CODE', 'CODEX'],
          onChange,
          unsupported,
        }}
        onEditRepos={() => {}}
        repoSummaryLabel="one repo"
        repoSummaryTitle="one repo"
      />
    );
  });
  return { onChange };
}

/** Open the agent dropdown and return its rendered option rows. */
function openExecutorMenu(): HTMLElement[] {
  const trigger = Array.from(container.querySelectorAll('button')).find(
    (button) => /claude/i.test(button.textContent ?? '')
  );
  if (!trigger) throw new Error('agent dropdown trigger not found');
  act(() => {
    trigger.dispatchEvent(
      new MouseEvent('pointerdown', { bubbles: true, button: 0 })
    );
    trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  });
  return Array.from(
    document.querySelectorAll('[role="menuitem"]')
  ) as HTMLElement[];
}

function findOption(options: HTMLElement[], label: RegExp) {
  const found = options.find((option) => label.test(option.textContent ?? ''));
  if (!found) {
    throw new Error(
      `no option matching ${label}; saw: ${options
        .map((o) => o.textContent)
        .join(' | ')}`
    );
  }
  return found;
}

describe('create-mode agent picker capability gating', () => {
  it('disables an unsupported agent and shows why', () => {
    renderPicker(new Map([['CODEX', 'Unavailable']]));
    const options = openExecutorMenu();

    const codex = findOption(options, /codex/i);
    expect(codex.getAttribute('data-disabled')).not.toBeNull();
    // The reason has to be visible text: a disabled row sets
    // pointer-events-none, so a title tooltip would never appear.
    expect(codex.textContent).toContain('Unavailable');

    const claude = findOption(options, /claude/i);
    expect(claude.getAttribute('data-disabled')).toBeNull();
  });

  it('leaves every agent enabled when the cluster has no opinion', () => {
    // FR-8: unparseable or absent capability data must not hide agents.
    renderPicker(undefined);
    const options = openExecutorMenu();

    for (const label of [/claude/i, /codex/i]) {
      expect(
        findOption(options, label).getAttribute('data-disabled')
      ).toBeNull();
    }
  });
});
