import { beforeEach, describe, expect, it, vi } from 'vitest';
import { confirmSettingsHostSwitch } from './settings/settingsHostSwitch';

const confirm = vi.hoisted(() => vi.fn());

vi.mock('@vibe/ui/components/ConfirmDialog', () => ({
  ConfirmDialog: {
    show: confirm,
  },
}));

describe('confirmSettingsHostSwitch', () => {
  const t = (key: string) => key;

  beforeEach(() => {
    confirm.mockReset();
  });

  it('preserves the current host and draft when the user cancels', async () => {
    confirm.mockResolvedValueOnce('cancelled');
    const clearAll = vi.fn();
    const setSelectedHostId = vi.fn();

    const changed = await confirmSettingsHostSwitch({
      isDirty: true,
      currentHostId: 'host-a',
      nextHostId: 'host-b',
      clearAll,
      setSelectedHostId,
      t,
    });

    expect(changed).toBe(false);
    expect(clearAll).not.toHaveBeenCalled();
    expect(setSelectedHostId).not.toHaveBeenCalled();
  });

  it('clears dirty state before switching after confirmation', async () => {
    confirm.mockResolvedValueOnce('confirmed');
    const calls: string[] = [];

    const changed = await confirmSettingsHostSwitch({
      isDirty: true,
      currentHostId: 'host-a',
      nextHostId: 'host-b',
      clearAll: () => calls.push('clear'),
      setSelectedHostId: (hostId) => calls.push(`set:${hostId}`),
      t,
    });

    expect(changed).toBe(true);
    expect(calls).toEqual(['clear', 'set:host-b']);
  });

  it('switches directly when there is no dirty draft', async () => {
    const setSelectedHostId = vi.fn();

    const changed = await confirmSettingsHostSwitch({
      isDirty: false,
      currentHostId: 'host-a',
      nextHostId: 'host-b',
      clearAll: vi.fn(),
      setSelectedHostId,
      t,
    });

    expect(changed).toBe(true);
    expect(confirm).not.toHaveBeenCalled();
    expect(setSelectedHostId).toHaveBeenCalledWith('host-b');
  });
});
