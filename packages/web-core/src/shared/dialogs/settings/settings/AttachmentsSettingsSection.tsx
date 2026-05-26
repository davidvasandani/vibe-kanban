import { useTranslation } from 'react-i18next';
import { CheckCircleIcon, WarningCircleIcon } from '@phosphor-icons/react';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { useStorageCapability } from '@/shared/hooks/useStorageCapability';
import { SettingsCard, SettingsField } from './SettingsComponents';

export function AttachmentsSettingsSection() {
  const { t } = useTranslation(['settings']);
  const runtime = useAppRuntime();
  const backend = runtime === 'remote' ? 'azure' : 'filesystem';
  const capability = useStorageCapability(backend);

  const statusLabel = capability.isLoading
    ? t('settings.attachments.status.loading', { ns: 'settings' })
    : capability.attachmentsEnabled
      ? t('settings.attachments.status.enabled', { ns: 'settings' })
      : t('settings.attachments.status.disabled', { ns: 'settings' });

  const backendLabel = t(`settings.attachments.backend.${backend}`, {
    ns: 'settings',
  });

  return (
    <SettingsCard
      title={t('settings.attachments.title', { ns: 'settings' })}
      description={t('settings.attachments.description', { ns: 'settings' })}
    >
      <SettingsField
        label={t('settings.attachments.backend.label', { ns: 'settings' })}
      >
        <div className="flex items-center gap-2 text-sm text-normal">
          {capability.isLoading ? null : capability.attachmentsEnabled ? (
            <CheckCircleIcon
              className="size-icon-sm text-success"
              weight="fill"
            />
          ) : (
            <WarningCircleIcon
              className="size-icon-sm text-warning"
              weight="fill"
            />
          )}
          <span>{backendLabel}</span>
          <span className="text-low">·</span>
          <span
            className={
              capability.attachmentsEnabled ? 'text-success' : 'text-warning'
            }
          >
            {statusLabel}
          </span>
        </div>
      </SettingsField>

      {!capability.isLoading &&
        !capability.attachmentsEnabled &&
        backend === 'azure' && (
          <div className="rounded-sm border border-border bg-secondary/40 p-3 space-y-2">
            <p className="text-sm text-normal">
              {t('settings.attachments.disabledHelp', { ns: 'settings' })}
            </p>
            <ul className="space-y-1 text-sm text-low font-mono">
              <li>
                {t('settings.attachments.envVars.azureAccount', {
                  ns: 'settings',
                })}
              </li>
              <li>
                {t('settings.attachments.envVars.azureKey', { ns: 'settings' })}
              </li>
              <li>
                {t('settings.attachments.envVars.azureContainer', {
                  ns: 'settings',
                })}
              </li>
            </ul>
          </div>
        )}
    </SettingsCard>
  );
}
