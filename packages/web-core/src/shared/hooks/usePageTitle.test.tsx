// @vitest-environment jsdom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { usePageTitle } from './usePageTitle';

function TitleHarness({ parts }: { parts: (string | null | undefined)[] }) {
  usePageTitle(...parts);
  return null;
}

describe('usePageTitle', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    globalThis.IS_REACT_ACT_ENVIRONMENT = false;
  });

  function render(parts: (string | null | undefined)[]) {
    act(() => root.render(<TitleHarness parts={parts} />));
  }

  it('uses a specific page title without concatenating context', () => {
    render(['Fix stale execution status', 'VAS', 'VAS-356']);

    expect(document.title).toBe('Fix stale execution status');
  });

  it('selects the first meaningful fallback', () => {
    render([undefined, '', 'Project name', 'Vibe Kanban']);

    expect(document.title).toBe('Project name');
  });

  it('skips whitespace-only candidates and trims the selected value', () => {
    render(['   ', '  Workspace name  ']);

    expect(document.title).toBe('Workspace name');
  });

  it('uses the product name when every candidate is absent', () => {
    render([null, undefined, '', '   ']);

    expect(document.title).toBe('Vibe Kanban');
  });

  it('replaces the previous title when candidates change', () => {
    render(['First issue', 'Project name']);
    render([undefined, 'Project name']);

    expect(document.title).toBe('Project name');
  });
});
