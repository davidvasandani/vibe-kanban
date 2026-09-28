/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  GITHUB_OWNER_PATTERN,
  GitHubOwnerTokensCard,
} from './GitHubOwnerTokensCard';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const client = vi.hoisted(() => ({
  queryScopeKey: ['machine', 'local'] as const,
  listGitHubOwnerTokens: vi.fn(),
  createGitHubOwnerToken: vi.fn(),
  updateGitHubOwnerToken: vi.fn(),
  deleteGitHubOwnerToken: vi.fn(),
}));
vi.mock('./SettingsHostContext', () => ({
  useSettingsMachineClient: () => client,
}));

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function change(input: HTMLInputElement, value: string) {
  act(() => {
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      'value'
    )!.set!.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

function input(placeholder: string) {
  return container.querySelector<HTMLInputElement>(
    `[placeholder="${placeholder}"]`
  )!;
}

function click(text: string) {
  const button = Array.from(container.querySelectorAll('button')).find(
    (button) => button.textContent?.trim() === text
  );
  expect(button).toBeDefined();
  act(() => button!.click());
}

beforeEach(async () => {
  vi.clearAllMocks();
  client.listGitHubOwnerTokens.mockResolvedValue([
    {
      id: 'literal',
      owner: 'sweetgreen',
      created_at: '',
      updated_at: '',
    },
    {
      id: 'reference',
      owner: 'davidvasandani',
      reference: 'op://Homelab/GitHub PAT/credential',
      created_at: '',
      updated_at: '',
    },
  ]);
  client.createGitHubOwnerToken.mockResolvedValue({});
  client.updateGitHubOwnerToken.mockResolvedValue({});
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  act(() =>
    root.render(
      <QueryClientProvider client={queryClient}>
        <GitHubOwnerTokensCard />
      </QueryClientProvider>
    )
  );
  for (
    let i = 0;
    i < 50 && !container.textContent?.includes('sweetgreen');
    i++
  ) {
    await flush();
  }
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('GitHubOwnerTokensCard', () => {
  it('masks literal tokens and shows references', () => {
    const text = container.textContent ?? '';
    expect(text).toContain('sweetgreen');
    expect(text).toContain('••••••••');
    expect(text).toContain('op://Homelab/GitHub PAT/credential');
  });

  it('submits a pasted reference without the copied quotes', async () => {
    change(input('GitHub owner'), 'BloopAI');
    change(
      input('token or op://vault/item/field'),
      '"op://Homelab/GitHub PAT/BloopAI"'
    );
    expect(input('token or op://vault/item/field').type).toBe('text');
    click('Add');
    await flush();
    expect(client.createGitHubOwnerToken).toHaveBeenCalledWith({
      owner: 'BloopAI',
      value: 'op://Homelab/GitHub PAT/BloopAI',
    });
  });

  it('keeps literal drafts hidden and rejects invalid or duplicate owners', async () => {
    change(input('token or op://vault/item/field'), 'synthetic-token');
    expect(input('token or op://vault/item/field').type).toBe('password');

    change(input('GitHub owner'), 'bad_owner');
    click('Add');
    change(input('GitHub owner'), 'SweetGreen');
    click('Add');
    await flush();
    expect(container.textContent).toContain('already exists');
    expect(client.createGitHubOwnerToken).not.toHaveBeenCalled();
  });

  it('edits start empty for literals and prefilled for references', () => {
    act(() =>
      container
        .querySelector<HTMLButtonElement>('[aria-label="Edit sweetgreen"]')!
        .click()
    );
    expect(input('New token or op://vault/item/field').value).toBe('');
    click('Cancel');
    act(() =>
      container
        .querySelector<HTMLButtonElement>('[aria-label="Edit davidvasandani"]')!
        .click()
    );
    expect(input('New token or op://vault/item/field').value).toBe(
      'op://Homelab/GitHub PAT/credential'
    );
  });

  it('owner pattern matches GitHub login rules', () => {
    for (const valid of ['a', 'Org-A', 'x'.repeat(39)]) {
      expect(GITHUB_OWNER_PATTERN.test(valid)).toBe(true);
    }
    for (const invalid of ['', '-a', 'a-', 'a_b', 'a.b', 'x'.repeat(40)]) {
      expect(GITHUB_OWNER_PATTERN.test(invalid)).toBe(false);
    }
  });
});
