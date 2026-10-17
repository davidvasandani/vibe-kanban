/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RemoteCloudHostsSettingsCardContent } from './RemoteCloudHostsSettingsCard';
import translations from '@/i18n/locales/en/settings.json';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const mocks = vi.hoisted(() => ({
  localList: vi.fn(),
  remoteList: vi.fn(),
  discover: vi.fn(),
  removeLocal: vi.fn(),
  removeRemote: vi.fn(),
  pairLocal: vi.fn(),
  pairRemote: vi.fn(),
  navigate: vi.fn(),
  close: vi.fn(),
  routeHostId: '',
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | Record<string, string>) => {
      let value: unknown = translations;
      for (const part of key.split('.'))
        value = (value as Record<string, unknown>)?.[part];
      let result =
        typeof value === 'string'
          ? value
          : typeof options === 'string'
            ? options
            : key;
      if (typeof options === 'object')
        for (const [name, val] of Object.entries(options))
          result = result.replace(`{{${name}}}`, val);
      return result;
    },
  }),
}));
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mocks.navigate,
  useParams: () => ({ hostId: mocks.routeHostId }),
}));
vi.mock('@/shared/hooks/useUserSystem', () => ({
  useUserSystem: () => ({ machineId: 'this-machine' }),
}));
vi.mock('@/shared/lib/api', () => ({
  relayApi: { listPairedRelayHosts: mocks.localList },
}));
vi.mock('@/shared/lib/relayClientIdentity', () => ({
  createRelayClientIdentity: () => ({ clientName: 'Browser' }),
}));
vi.mock('@/shared/lib/relayPake', () => ({
  normalizeEnrollmentCode: (code: string) =>
    code.replaceAll('-', '').toUpperCase(),
}));
vi.mock('@/shared/hooks/useRemoteCloudHosts', () => ({
  usePairRemoteCloudHostMutation: () => ({
    mutateAsync: mocks.pairLocal,
    isPending: false,
  }),
  useRemoveRemoteCloudHostMutation: () => ({
    mutateAsync: mocks.removeLocal,
    isPending: false,
  }),
}));
vi.mock('./useRelayRemoteHostMutations', () => ({
  useRelayRemoteHostsQuery: () => ({
    queryKey: ['live'],
    queryFn: mocks.discover,
  }),
  useRelayRemotePairedHostsQuery: () => ({
    queryKey: ['paired'],
    queryFn: mocks.remoteList,
  }),
  usePairRelayHostMutation: () => ({
    mutateAsync: mocks.pairRemote,
    isPending: false,
  }),
  useRemovePairedRelayHostMutation: () => ({
    mutateAsync: mocks.removeRemote,
    isPending: false,
  }),
}));
vi.mock('@vibe/ui/components/PrimaryButton', () => ({
  PrimaryButton: ({
    value,
    onClick,
    disabled,
  }: {
    value: string;
    onClick: () => void;
    disabled?: boolean;
  }) => (
    <button onClick={onClick} disabled={disabled}>
      {value}
    </button>
  ),
}));
vi.mock('./SettingsComponents', () => ({
  SettingsField: ({
    label,
    children,
  }: {
    label: string;
    children: React.ReactNode;
  }) => (
    <label>
      {label}
      {children}
    </label>
  ),
  SettingsInput: ({
    value,
    onChange,
  }: {
    value: string;
    onChange: (v: string) => void;
  }) => <input value={value} onChange={(e) => onChange(e.target.value)} />,
  SettingsSelect: ({
    value,
    options,
    onChange,
  }: {
    value: string;
    options: { value: string; label: string }[];
    onChange: (v: string) => void;
  }) => (
    <select value={value} onChange={(e) => onChange(e.target.value)}>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  ),
}));
vi.mock('./PairingCodeInput', () => ({
  PairingCodeInput: () => <input aria-label="Pairing code" />,
}));

let container: HTMLDivElement;
let root: Root;
let client: QueryClient;
const paired = [
  {
    host_id: 'mac-id',
    host_name: 'Mac mini',
    paired_at: '2026-09-16T10:00:00Z',
  },
];
const button = (text: string) =>
  Array.from(container.querySelectorAll('button')).find(
    (b) => b.textContent === text
  )!;
async function render(
  mode: 'local' | 'remote' = 'local',
  showPairing = false,
  initialHostId?: string
) {
  await act(async () => {
    root.render(
      <QueryClientProvider client={client}>
        <RemoteCloudHostsSettingsCardContent
          mode={mode}
          showPairing={showPairing}
          initialHostId={initialHostId}
          onClose={mocks.close}
        />
      </QueryClientProvider>
    );
  });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}
async function click(text: string) {
  await act(async () => {
    button(text).click();
  });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}
beforeEach(() => {
  vi.resetAllMocks();
  mocks.routeHostId = '';
  mocks.localList.mockResolvedValue(paired);
  mocks.remoteList.mockResolvedValue(paired);
  mocks.discover.mockResolvedValue([]);
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
  client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
});
afterEach(async () => {
  await act(async () => root.unmount());
  client.clear();
  container.remove();
  vi.restoreAllMocks();
});

describe.each(['local', 'remote'] as const)(
  '%s remote machine management',
  (mode) => {
    it('shows and removes offline pairings with no discovery candidates', async () => {
      await render(mode);
      expect(container.textContent).toContain('Mac mini');
      expect(container.textContent).toContain('Offline');
      expect(button('Open workspaces').disabled).toBe(true);
      await click('Remove');
      expect(window.confirm).toHaveBeenCalledWith(
        expect.stringContaining('Mac mini')
      );
      expect(
        mode === 'local' ? mocks.removeLocal : mocks.removeRemote
      ).toHaveBeenCalledWith('mac-id');
      expect(
        mode === 'local' ? mocks.remoteList : mocks.localList
      ).not.toHaveBeenCalled();
    });
    it('keeps identities visible with unknown status when discovery fails', async () => {
      mocks.discover.mockRejectedValue(new Error('network unavailable'));
      await render(mode);
      expect(container.textContent).toContain('Mac mini');
      expect(container.textContent).toContain('Status unknown');
      expect(container.querySelector('[role="alert"]')?.textContent).toContain(
        'Could not check'
      );
      expect(button('Open workspaces').disabled).toBe(true);
      expect(button('Remove').disabled).toBe(false);
    });
    it('reports inventory failures without a false empty state and refreshes', async () => {
      const list = mode === 'local' ? mocks.localList : mocks.remoteList;
      list.mockRejectedValue(new Error('storage unavailable'));
      await render(mode);
      expect(container.textContent).toContain('Could not load paired machines');
      expect(container.textContent).not.toContain('No machines paired yet');
      list.mockResolvedValue(paired);
      await click('Refresh');
      expect(container.textContent).toContain('Mac mini');
      expect(container.textContent).not.toContain(
        'Could not load paired machines'
      );
    });
    it('opens online workspaces by host identity and closes settings', async () => {
      mocks.discover.mockResolvedValue([
        { id: 'mac-id', name: 'Live Mac', status: 'online' },
      ]);
      await render(mode);
      await click('Open workspaces');
      expect(mocks.close).toHaveBeenCalledOnce();
      expect(mocks.navigate).toHaveBeenCalledWith({
        to: '/hosts/$hostId/workspaces',
        params: { hostId: 'mac-id' },
      });
    });
    it('does not open a host that requires pairing', async () => {
      mocks.discover.mockResolvedValue([
        { id: 'mac-id', name: 'Mac', status: 'unpaired' },
      ]);
      await render(mode);
      expect(container.textContent).toContain('Pairing required');
      expect(button('Open workspaces').disabled).toBe(true);
    });
    it('cancels removal without mutation and shows mutation failures', async () => {
      await render(mode);
      vi.mocked(window.confirm).mockReturnValue(false);
      await click('Remove');
      const remove = mode === 'local' ? mocks.removeLocal : mocks.removeRemote;
      expect(remove).not.toHaveBeenCalled();
      vi.mocked(window.confirm).mockReturnValue(true);
      remove.mockRejectedValue(new Error('Removal failed'));
      await click('Remove');
      expect(container.textContent).toContain('Removal failed');
      expect(container.textContent).toContain('Mac mini');
    });
    it('returns home when removing the active host', async () => {
      mocks.routeHostId = 'mac-id';
      await render(mode);
      await click('Remove');
      expect(mocks.navigate).toHaveBeenCalledWith({ to: '/' });
      expect(mocks.close).toHaveBeenCalledOnce();
    });
    it('preserves initial pairing selection while inventory is visible', async () => {
      mocks.discover.mockResolvedValue([
        { id: 'other', name: 'Other', status: 'online' },
        { id: 'mac-id', name: 'Mac', status: 'online' },
      ]);
      await render(mode, true, 'mac-id');
      expect(container.querySelector('select')?.value).toBe('mac-id');
      expect(button('Remove')).toBeDefined();
    });
  }
);
