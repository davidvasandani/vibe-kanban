import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import type { SettingsHostTargetId } from './SettingsHostContext';

export async function confirmSettingsHostSwitch({
  isDirty,
  currentHostId,
  nextHostId,
  clearAll,
  setSelectedHostId,
  t,
}: {
  isDirty: boolean;
  currentHostId: SettingsHostTargetId | null;
  nextHostId: SettingsHostTargetId;
  clearAll: () => void;
  setSelectedHostId: (hostId: SettingsHostTargetId) => void;
  t: (key: string) => string;
}): Promise<boolean> {
  if (nextHostId === currentHostId) {
    return false;
  }

  if (isDirty) {
    const result = await ConfirmDialog.show({
      title: t('settings.unsavedChanges.hostSwitchTitle'),
      message: t('settings.unsavedChanges.hostSwitchMessage'),
      confirmText: t('settings.unsavedChanges.discard'),
      cancelText: t('settings.unsavedChanges.cancel'),
      variant: 'destructive',
    });
    if (result !== 'confirmed') {
      return false;
    }
    clearAll();
  }

  setSelectedHostId(nextHostId);
  return true;
}
