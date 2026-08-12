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
