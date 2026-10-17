/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Navbar } from '@vibe/ui/components/Navbar';

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

describe('Navbar mobile right sidebar tab', () => {
  it('expands workspace tools while keeping trailing actions fixed', () => {
    act(() => {
      root.render(
        <Navbar
          mobileMode
          mobileActiveTab="chat"
          onOpenDrawer={vi.fn()}
          onOpenSettings={vi.fn()}
          onOpenCommandBar={vi.fn()}
        />
      );
    });

    const toolbar = container.querySelector(
      '[data-testid="mobile-workspace-toolbar"]'
    );
    const tabs = container.querySelector(
      '[data-testid="mobile-workspace-tabs"]'
    );
    const actions = container.querySelector(
      '[data-testid="mobile-navbar-actions"]'
    );
    const chatTab = container.querySelector('[aria-label="Chat"]');
    const projectsButton = container.querySelector('[aria-label="Projects"]');

    expect(toolbar).not.toBeNull();
    expect(toolbar?.classList.contains('flex-1')).toBe(true);
    expect(toolbar?.classList.contains('min-w-0')).toBe(true);
    expect(toolbar?.classList.contains('overflow-x-auto')).toBe(false);
    expect(tabs?.classList.contains('flex-1')).toBe(true);
    expect(tabs?.classList.contains('min-w-0')).toBe(true);
    expect(tabs?.classList.contains('overflow-x-auto')).toBe(true);
    expect(projectsButton?.classList.contains('shrink-0')).toBe(true);
    expect(chatTab?.classList.contains('flex-1')).toBe(true);
    expect(chatTab?.classList.contains('min-w-10')).toBe(true);
    expect(actions?.classList.contains('shrink-0')).toBe(true);
    expect(chatTab?.getAttribute('aria-pressed')).toBe('true');
  });

  it('identifies, selects, and activates the existing drawer destination', () => {
    const onMobileTabChange = vi.fn();

    act(() => {
      root.render(
        <Navbar
          mobileMode
          mobileActiveTab="git"
          onMobileTabChange={onMobileTabChange}
        />
      );
    });

    const button = container.querySelector('[aria-label="Right sidebar"]');
    if (!(button instanceof HTMLButtonElement)) {
      throw new Error('Expected a right sidebar tab button');
    }

    expect(button.getAttribute('aria-pressed')).toBe('true');
    expect(button.textContent).toContain('Sidebar');
    expect(button.querySelector('svg')?.style.transform).toBe('scaleX(-1)');

    act(() => button.click());

    expect(onMobileTabChange).toHaveBeenCalledOnce();
    expect(onMobileTabChange).toHaveBeenCalledWith('git');
  });

  it('reports the drawer destination as unselected on another tab', () => {
    act(() => {
      root.render(<Navbar mobileMode mobileActiveTab="chat" />);
    });

    expect(
      container
        .querySelector('[aria-label="Right sidebar"]')
        ?.getAttribute('aria-pressed')
    ).toBe('false');
  });
});
