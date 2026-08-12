/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

function sectionRoot() {
  const element = container.firstElementChild;
  if (!(element instanceof HTMLDivElement)) {
    throw new Error('Expected a section root');
  }
  return element;
}

beforeEach(() => {
  window.localStorage.clear();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  window.localStorage.clear();
});

describe('CollapsibleSectionHeader flexible sizing', () => {
  it('grows while expanded and returns to intrinsic height when collapsed', () => {
    act(() => {
      root.render(
        <CollapsibleSectionHeader title="Git" fillAvailableSpace>
          <div>Repository content</div>
        </CollapsibleSectionHeader>
      );
    });

    expect(sectionRoot().classList).toContain('flex-1');
    expect(sectionRoot().classList).toContain('min-h-0');
    expect(sectionRoot().textContent).toContain('Repository content');

    const toggle = container.querySelector('button');
    if (!(toggle instanceof HTMLButtonElement)) {
      throw new Error('Expected a collapsible section button');
    }

    act(() => toggle.click());

    expect(sectionRoot().classList).toContain('flex-none');
    expect(sectionRoot().classList).toContain('h-auto');
    expect(sectionRoot().classList).not.toContain('flex-1');
    expect(sectionRoot().textContent).not.toContain('Repository content');
  });

  it('preserves the existing root sizing when flexible sizing is omitted', () => {
    act(() => {
      root.render(
        <CollapsibleSectionHeader title="Notes">
          <div>Notes content</div>
        </CollapsibleSectionHeader>
      );
    });

    expect(sectionRoot().classList).toContain('h-full');
    expect(sectionRoot().classList).toContain('min-h-0');
    expect(sectionRoot().classList).not.toContain('flex-1');
    expect(sectionRoot().classList).not.toContain('flex-none');
  });

  it('keeps an expanded intrinsic section at its content height', () => {
    act(() => {
      root.render(
        <CollapsibleSectionHeader title="Affinity" intrinsicHeight>
          <div>Affinity controls</div>
        </CollapsibleSectionHeader>
      );
    });

    expect(sectionRoot().classList).toContain('flex-none');
    expect(sectionRoot().classList).toContain('h-auto');
    expect(sectionRoot().classList).not.toContain('h-full');
    expect(sectionRoot().classList).not.toContain('flex-1');
    expect(sectionRoot().textContent).toContain('Affinity controls');
  });

  it('keeps a non-collapsible section at its intrinsic height', () => {
    act(() => {
      root.render(
        <CollapsibleSectionHeader
          title="Issue"
          collapsible={false}
          fillAvailableSpace
        >
          <div>Issue content</div>
        </CollapsibleSectionHeader>
      );
    });

    expect(sectionRoot().classList).toContain('flex-none');
    expect(sectionRoot().classList).toContain('h-auto');
    expect(sectionRoot().classList).not.toContain('flex-1');
    expect(container.querySelector('button')).toBeNull();
    expect(sectionRoot().textContent).toContain('Issue content');
  });

  it('keeps header context visible when the section is collapsed', () => {
    act(() => {
      root.render(
        <CollapsibleSectionHeader
          title="Server Affinity"
          headerExtra={<span>think4</span>}
          fillAvailableSpace
        >
          <div>Affinity controls</div>
        </CollapsibleSectionHeader>
      );
    });

    const toggle = container.querySelector('button');
    if (!(toggle instanceof HTMLButtonElement)) {
      throw new Error('Expected a collapsible section button');
    }

    act(() => toggle.click());

    expect(sectionRoot().textContent).toContain('think4');
    expect(sectionRoot().textContent).not.toContain('Affinity controls');
  });
});
