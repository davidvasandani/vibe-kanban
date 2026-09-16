/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import {
  GlobalSearchDialog,
  useGlobalSearchShortcut,
} from './GlobalSearchDialog';
import {
  searchGlobally,
  type GlobalSearchResponse,
} from '@/shared/lib/globalSearch';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/shared/stores/useOrganizationStore', () => ({
  useOrganizationStore: () => vi.fn(),
}));
vi.mock('@/shared/lib/globalSearch', async (original) => ({
  ...(await original<object>()),
  searchGlobally: vi.fn(),
}));
vi.mock('@/shared/lib/remoteApi', () => ({}));
vi.mock('@/shared/lib/localApiTransport', () => ({}));
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
let root: ReturnType<typeof createRoot>;
let container: HTMLDivElement;
beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
  );
  Element.prototype.scrollIntoView = vi.fn();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});
afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
function typeQuery(value: string) {
  const input = document.querySelector<HTMLInputElement>(
    '[aria-label="Global search query"]'
  )!;
  act(() => {
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      'value'
    )!.set!.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}
it('debounces input and ignores a stale response while showing incomplete coverage', async () => {
  let finishOld!: (value: GlobalSearchResponse) => void;
  vi.mocked(searchGlobally)
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishOld = resolve;
        })
    )
    .mockResolvedValueOnce({
      results: [],
      truncated: false,
      unavailable: ['Offline host'],
    });
  act(() =>
    root.render(
      <GlobalSearchDialog open onOpenChange={vi.fn()} local remote={false} />
    )
  );
  expect(searchGlobally).not.toHaveBeenCalled();
  typeQuery('old');
  await act(async () => vi.advanceTimersByTimeAsync(250));
  typeQuery('new');
  await act(async () => vi.advanceTimersByTimeAsync(250));
  expect(document.body.textContent).toContain('Offline host');
  await act(async () =>
    finishOld({ results: [], truncated: true, unavailable: ['Stale host'] })
  );
  expect(document.body.textContent).toContain('Offline host');
  expect(document.body.textContent).not.toContain('Stale host');
});

it('lets the shell coordinate an organization change before navigating a result', async () => {
  const selectOrg = vi.fn();
  vi.mocked(searchGlobally).mockResolvedValue({
    results: [
      {
        kind: 'project',
        id: 'p',
        title: 'Other project',
        context: 'Other org',
        snippet: '',
        archived: false,
        project_id: 'p',
        organization_id: 'org',
      },
    ],
    truncated: false,
    unavailable: [],
  });
  act(() =>
    root.render(
      <GlobalSearchDialog
        open
        onOpenChange={vi.fn()}
        local
        remote
        onSelectOrganization={selectOrg}
      />
    )
  );
  typeQuery('other');
  await act(async () => vi.advanceTimersByTimeAsync(250));
  const item = document.querySelector<HTMLElement>('[cmdk-item]')!;
  act(() => item.click());
  expect(selectOrg).toHaveBeenCalledWith('org');
});

it('shows issue IDs in their own group and coordinates selection', async () => {
  const selectOrg = vi.fn();
  vi.mocked(searchGlobally).mockResolvedValue({
    results: [
      {
        kind: 'issue',
        id: 'issue-id',
        title: 'Searching by Issue ID should work',
        context: 'VAS-602 · Vibe Kanban / Vibe Kanban',
        snippet: '',
        archived: false,
        project_id: 'project-id',
        issue_id: 'issue-id',
        organization_id: 'org-id',
      },
    ],
    truncated: false,
    unavailable: [],
  });
  act(() =>
    root.render(
      <GlobalSearchDialog
        open
        onOpenChange={vi.fn()}
        local
        remote
        onSelectOrganization={selectOrg}
      />
    )
  );
  typeQuery('vas-602');
  await act(async () => vi.advanceTimersByTimeAsync(250));
  expect(document.body.textContent).toContain('Issues');
  expect(document.body.textContent).toContain(
    'Searching by Issue ID should work'
  );
  expect(document.body.textContent).toContain('VAS-602');
  const item = document.querySelector<HTMLElement>('[cmdk-item]')!;
  act(() => item.click());
  expect(selectOrg).toHaveBeenCalledWith('org-id');
});

it('opens from Ctrl+Shift+F without intercepting ordinary browser find', () => {
  const open = vi.fn();
  function Shortcut() {
    useGlobalSearchShortcut(open);
    return null;
  }
  act(() => root.render(<Shortcut />));
  const searchKey = new KeyboardEvent('keydown', {
    key: 'F',
    ctrlKey: true,
    shiftKey: true,
    cancelable: true,
  });
  act(() => window.dispatchEvent(searchKey));
  expect(open).toHaveBeenCalledOnce();
  expect(searchKey.defaultPrevented).toBe(true);
  act(() =>
    window.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'f',
        ctrlKey: true,
        cancelable: true,
      })
    )
  );
  expect(open).toHaveBeenCalledOnce();
});
