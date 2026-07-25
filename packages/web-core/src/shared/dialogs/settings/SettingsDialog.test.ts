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
    const setSelectedHostId = vi.fn();

    const changed = await confirmSettingsHostSwitch({
      isDirty: true,
      currentHostId: 'host-a',
      nextHostId: 'host-b',
      setSelectedHostId,
      t,
    });

    expect(changed).toBe(false);
    expect(setSelectedHostId).not.toHaveBeenCalled();
  });

  it('switches after confirmation without clearing unrelated dirty state', async () => {
    confirm.mockResolvedValueOnce('confirmed');
    const calls: string[] = [];

    const changed = await confirmSettingsHostSwitch({
      isDirty: true,
      currentHostId: 'host-a',
      nextHostId: 'host-b',
      setSelectedHostId: (hostId) => calls.push(`set:${hostId}`),
      t,
    });

    expect(changed).toBe(true);
    expect(calls).toEqual(['set:host-b']);
  });

  it('switches directly when there is no dirty draft', async () => {
    const setSelectedHostId = vi.fn();

    const changed = await confirmSettingsHostSwitch({
      isDirty: false,
      currentHostId: 'host-a',
      nextHostId: 'host-b',
      setSelectedHostId,
      t,
    });

    expect(changed).toBe(true);
    expect(confirm).not.toHaveBeenCalled();
    expect(setSelectedHostId).toHaveBeenCalledWith('host-b');
  });
});
