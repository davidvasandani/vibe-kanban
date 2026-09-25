/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { OrganizationEnvVarsCard } from './OrganizationEnvVarsCard';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mutations = vi.hoisted(() => ({ create: vi.fn(), update: vi.fn() }));
vi.mock('@/shared/hooks/useOrganizationEnvVars', () => ({
  useOrganizationEnvVars: () => ({
    data: [{ id: 'existing', name: 'API_TOKEN' }],
    isLoading: false,
  }),
  useOrganizationEnvVarMutations: () => ({
    createEnvVar: { mutate: mutations.create, isPending: false },
    updateEnvVar: { mutate: mutations.update, isPending: false },
    deleteEnvVar: { mutate: vi.fn(), isPending: false },
  }),
}));

let container: HTMLDivElement;
let root: Root;

function change(input: HTMLInputElement, value: string) {
  act(() => {
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      'value'
    )!.set!.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

function click(text: string) {
  const button = Array.from(container.querySelectorAll('button')).find(
    (button) => button.textContent?.trim() === text
  );
  expect(button).toBeDefined();
  act(() => button!.click());
}

beforeEach(() => {
  vi.clearAllMocks();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  act(() => root.render(<OrganizationEnvVarsCard organizationId="org" />));
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe.each(['add', 'edit'] as const)('%s environment variable', (mode) => {
  function valueInput() {
    if (mode === 'edit') {
      act(() =>
        container
          .querySelector<HTMLButtonElement>('[aria-label="Edit API_TOKEN"]')!
          .click()
      );
    }
    return container.querySelector<HTMLInputElement>(
      mode === 'add'
        ? '[placeholder="value or op://vault/item/field"]'
        : '[placeholder="New value or op://vault/item/field"]'
    )!;
  }

  it('shows reference drafts and remasks when the prefix is removed or replaced', () => {
    const input = valueInput();
    expect(input.type).toBe('password');
    for (const value of ['o', 'op', 'op:', 'op:/']) {
      change(input, value);
      expect(input.type).toBe('password');
    }
    for (const value of ['op://', 'op://Team Vault/Test Item/password']) {
      change(input, value);
      expect(input.type).toBe('text');
      expect(input.value).toBe(value);
    }
    for (const value of [
      'Team Vault/Test Item/password',
      'synthetic-secret',
      'OP://vault/item/field',
      ' op://vault/item/field',
      'prefix-op://vault/item/field',
      '',
    ]) {
      change(input, 'op://vault/item/field');
      change(input, value);
      expect(input.type).toBe('password');
      expect(input.value).toBe(value);
    }
  });

  it.each(['op://Team Vault/Test Item/password', ' synthetic-secret '])(
    'submits the original value without transformation: %s',
    (value) => {
      const input = valueInput();
      change(input, value);
      if (mode === 'add') {
        change(
          container.querySelector<HTMLInputElement>('[placeholder="NAME"]')!,
          'NEW_TOKEN'
        );
        click('Add');
        expect(mutations.create).toHaveBeenCalledWith(
          { name: 'NEW_TOKEN', value },
          expect.objectContaining({ onSuccess: expect.any(Function) })
        );
      } else {
        click('Save');
        expect(mutations.update).toHaveBeenCalledWith(
          { id: 'existing', value },
          expect.objectContaining({ onSuccess: expect.any(Function) })
        );
      }
    }
  );
});

it('keeps saved rows redacted and discards the reference draft on cancel', () => {
  expect(container.textContent).toContain('••••••••');
  act(() =>
    container
      .querySelector<HTMLButtonElement>('[aria-label="Edit API_TOKEN"]')!
      .click()
  );
  const input = container.querySelector<HTMLInputElement>(
    '[placeholder="New value or op://vault/item/field"]'
  )!;
  change(input, 'op://vault/item/field');
  click('Cancel');
  expect(container.textContent).toContain('••••••••');
  expect(
    container.querySelector(
      '[placeholder="New value or op://vault/item/field"]'
    )
  ).toBeNull();
  act(() =>
    container
      .querySelector<HTMLButtonElement>('[aria-label="Edit API_TOKEN"]')!
      .click()
  );
  const reopened = container.querySelector<HTMLInputElement>(
    '[placeholder="New value or op://vault/item/field"]'
  )!;
  expect(reopened.value).toBe('');
  expect(reopened.type).toBe('password');
  expect(mutations.update).not.toHaveBeenCalled();
});
