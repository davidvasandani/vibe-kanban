/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  OrganizationCommitIdentityCard,
  parseAllowedEmails,
} from './OrganizationCommitIdentityCard';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const state = vi.hoisted(() => ({
  identity: null as null | {
    organization_id: string;
    name: string;
    email: string;
    allowed_emails: string[];
    updated_at: string;
  },
  save: vi.fn(),
  remove: vi.fn(),
}));
vi.mock('@/shared/hooks/useOrganizationCommitIdentity', () => ({
  useOrganizationCommitIdentity: () => ({
    data: state.identity,
    isLoading: false,
  }),
  useOrganizationCommitIdentityMutations: () => ({
    saveCommitIdentity: { mutate: state.save, isPending: false },
    removeCommitIdentity: { mutate: state.remove, isPending: false },
  }),
}));

let container: HTMLDivElement;
let root: Root;

function render() {
  act(() =>
    root.render(<OrganizationCommitIdentityCard organizationId="org" />)
  );
}

function setValue(
  element: HTMLInputElement | HTMLTextAreaElement,
  value: string
) {
  act(() => {
    const proto =
      element instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value')!.set!.call(element, value);
    element.dispatchEvent(new Event('input', { bubbles: true }));
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
  state.identity = null;
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('OrganizationCommitIdentityCard', () => {
  it('saves a trimmed identity and allowlist', () => {
    render();
    setValue(
      container.querySelector<HTMLInputElement>('[placeholder="Jane Doe"]')!,
      ' David Vasandani '
    );
    setValue(
      container.querySelector<HTMLInputElement>(
        '[placeholder="jane@example.com"]'
      )!,
      'david@vasandani.me '
    );
    setValue(
      container.querySelector<HTMLTextAreaElement>('textarea')!,
      'noreply@anthropic.com\n\n noreply@vibekanban.com, '
    );
    click('Save');
    expect(state.save).toHaveBeenCalledWith({
      name: 'David Vasandani',
      email: 'david@vasandani.me',
      allowed_emails: ['noreply@anthropic.com', 'noreply@vibekanban.com'],
    });
  });

  it('rejects an invalid allowlisted email without saving', () => {
    render();
    setValue(
      container.querySelector<HTMLInputElement>('[placeholder="Jane Doe"]')!,
      'A'
    );
    setValue(
      container.querySelector<HTMLInputElement>(
        '[placeholder="jane@example.com"]'
      )!,
      'a@b.c'
    );
    setValue(
      container.querySelector<HTMLTextAreaElement>('textarea')!,
      'not an email'
    );
    click('Save');
    expect(state.save).not.toHaveBeenCalled();
    expect(container.textContent).toContain('"not an email"');
  });

  it('prefills a saved identity and can remove it', () => {
    state.identity = {
      organization_id: 'org',
      name: 'Org Owner',
      email: 'owner@org.example',
      allowed_emails: ['noreply@anthropic.com'],
      updated_at: '2026-11-23T00:00:00Z',
    };
    render();
    expect(
      container.querySelector<HTMLInputElement>(
        '[placeholder="jane@example.com"]'
      )!.value
    ).toBe('owner@org.example');
    expect(container.querySelector('textarea')!.value).toBe(
      'noreply@anthropic.com'
    );
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    click('Remove');
    expect(state.remove).toHaveBeenCalled();
  });
});

describe('parseAllowedEmails', () => {
  it('splits on newlines and commas and drops blanks', () => {
    expect(parseAllowedEmails(' a@b.c ,\n\nd@e.f\n')).toEqual([
      'a@b.c',
      'd@e.f',
    ]);
  });
});
