import { useTranslation } from 'react-i18next';
import type { BaseCodingAgent } from 'shared/types';
import type { DisableAgentBlocker } from '@/shared/lib/disabledAgents';
import { SettingsCard, SettingsCheckbox } from './SettingsComponents';

/** Whether an agent is offered in agent pickers. */
export function AgentVisibilityCard({
  executor,
  agentDisabled,
  blocker,
  onChange,
  disabled,
}: {
  executor: BaseCodingAgent;
  agentDisabled: boolean;
  blocker: DisableAgentBlocker | null;
  onChange: (disabled: boolean) => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation(['settings']);
  const lockedReason =
    blocker === 'default'
      ? t('settings.agents.visibility.defaultLocked')
      : blocker === 'last'
        ? t('settings.agents.visibility.lastLocked')
        : undefined;

  return (
    <SettingsCard
      title={t('settings.agents.visibility.title')}
      description={t('settings.agents.visibility.description')}
    >
      <SettingsCheckbox
        id={`agent-visible-${executor}`}
        label={t('settings.agents.visibility.label')}
        description={lockedReason}
        checked={!agentDisabled}
        disabled={disabled || blocker !== null}
        onChange={(checked) => onChange(!checked)}
      />
    </SettingsCard>
  );
}
