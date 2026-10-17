/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { RelaySettingsSectionContent } from './RelaySettingsSection';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const state = vi.hoisted(() => ({
  runtime: 'local',
  signedIn: true,
  setDirty: vi.fn(),
  config: { relay_enabled: false },
}));
vi.mock('@/shared/hooks/useAppRuntime', () => ({
  useAppRuntime: () => state.runtime,
}));
vi.mock('@/shared/hooks/auth/useAuth', () => ({
  useAuth: () => ({ isSignedIn: state.signedIn }),
}));
vi.mock('@/shared/hooks/useUserSystem', () => ({
  useUserSystem: () => ({ config: state.config, loading: false }),
}));
vi.mock('./SettingsDirtyContext', () => ({
  useSettingsDirty: () => ({ setDirty: state.setDirty }),
}));
vi.mock('@/shared/dialogs/global/OAuthDialog', () => ({
  OAuthDialog: { show: vi.fn() },
}));
vi.mock('@/shared/lib/api', () => ({ relayApi: {} }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));
vi.mock('@vibe/ui/components/PrimaryButton', () => ({
  PrimaryButton: ({ value }: { value: string }) => <button>{value}</button>,
}));
vi.mock('./SettingsComponents', () => ({
  SettingsCard: ({
    title,
    children,
  }: {
    title: string;
    children: React.ReactNode;
  }) => (
    <section>
      <h2>{title}</h2>
      {children}
    </section>
  ),
  SettingsSaveBar: () => null,
  SettingsInput: () => null,
  SettingsField: () => null,
  SettingsCheckbox: () => null,
}));
vi.mock('./RemoteCloudHostsSettingsCard', () => ({
  RemoteCloudHostsSettingsCardContent: ({
    showPairing = true,
    initialHostId,
    mode = 'local',
  }: {
    showPairing?: boolean;
    initialHostId?: string;
    mode?: string;
  }) => (
    <div data-inventory={mode} data-target={initialHostId}>
      {showPairing ? 'Pairing visible' : 'Inventory only'}
    </div>
  ),
}));
let container: HTMLDivElement;
let root: Root;
let client: QueryClient;
beforeEach(() => {
  state.runtime = 'local';
  state.signedIn = true;
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});
afterEach(async () => {
  await act(async () => root.unmount());
  client.clear();
  container.remove();
});
async function render(hostId?: string) {
  await act(async () => {
    root.render(
      <QueryClientProvider client={client}>
        <RelaySettingsSectionContent
          initialState={hostId ? { hostId } : undefined}
        />
      </QueryClientProvider>
    );
  });
}
it('shows one inventory before choosing a role and keeps Client/Host setup available', async () => {
  await render();
  expect(container.querySelectorAll('[data-inventory]')).toHaveLength(1);
  expect(container.textContent).toContain('Inventory only');
  const clientButton = Array.from(container.querySelectorAll('button')).find(
    (b) => b.textContent?.startsWith('Client')
  )!;
  await act(async () => clientButton.click());
  expect(container.textContent).toContain('Pairing visible');
  expect(container.querySelectorAll('[data-inventory]')).toHaveLength(1);
  const hostButton = Array.from(container.querySelectorAll('button')).find(
    (b) => b.textContent?.startsWith('Host')
  )!;
  await act(async () => hostButton.click());
  expect(container.textContent).toContain('Accept incoming connections');
  expect(container.textContent).toContain('Inventory only');
});
it.each(['local', 'remote'])(
  'preserves initial pairing target in %s runtime',
  async (runtime) => {
    state.runtime = runtime;
    await render('target-host');
    expect(
      container.querySelector('[data-inventory]')?.getAttribute('data-target')
    ).toBe('target-host');
    expect(
      container
        .querySelector('[data-inventory]')
        ?.getAttribute('data-inventory')
    ).toBe(runtime);
    expect(container.textContent).toContain('Pairing visible');
  }
);
it.each(['local', 'remote'])(
  'does not mount inventory queries before sign-in in %s runtime',
  async (runtime) => {
    state.runtime = runtime;
    state.signedIn = false;
    await render();
    expect(container.querySelector('[data-inventory]')).toBeNull();
    expect(container.textContent).toContain('Sign in');
  }
);
