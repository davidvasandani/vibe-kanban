import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import type { AutoErrorRemediationConfig, BaseCodingAgent } from 'shared/types';
import { repoApi } from '@/shared/lib/api';
import { toPrettyCase } from '@/shared/lib/string';
import { useAllOrganizationProjects } from '@/shared/hooks/useAllOrganizationProjects';
import {
  SettingsCard,
  SettingsCheckbox,
  SettingsField,
  SettingsInput,
  SettingsSelect,
} from './SettingsComponents';

// Sentinel select value for "use the failing workspace's own project".
const SOURCE_PROJECT = '__source__';

interface AutoErrorRemediationSettingsCardProps {
  value: AutoErrorRemediationConfig;
  executors: string[];
  onChange: (next: AutoErrorRemediationConfig) => void;
}

/**
 * Settings for auto error remediation: when a coding-agent turn fails, file
 * an issue with pipelines attached and start an unattended workspace on it.
 * `onChange` receives the whole config so array edits replace rather than
 * deep-merge.
 */
export function AutoErrorRemediationSettingsCard({
  value,
  executors,
  onChange,
}: AutoErrorRemediationSettingsCardProps) {
  const { t } = useTranslation(['settings']);
  const { data: projects } = useAllOrganizationProjects();
  const { data: repos = [] } = useQuery({
    queryKey: ['repos'],
    queryFn: () => repoApi.list(),
  });

  const update = (patch: Partial<AutoErrorRemediationConfig>) =>
    onChange({ ...value, ...patch });

  const toggleRepo = (repoId: string, checked: boolean) =>
    update({
      repo_ids: checked
        ? [...value.repo_ids, repoId]
        : value.repo_ids.filter((id) => id !== repoId),
    });

  const projectOptions = [
    {
      value: SOURCE_PROJECT,
      label: t('settings.general.autoRemediation.project.source'),
    },
    ...projects.map((project) => ({ value: project.id, label: project.name })),
  ];

  return (
    <SettingsCard
      title={t('settings.general.autoRemediation.title')}
      description={t('settings.general.autoRemediation.description')}
    >
      <SettingsCheckbox
        id="auto-error-remediation-enabled"
        label={t('settings.general.autoRemediation.enabled.label')}
        description={t('settings.general.autoRemediation.enabled.helper')}
        checked={value.enabled}
        onChange={(enabled) => update({ enabled })}
      />

      <SettingsField
        label={t('settings.general.autoRemediation.project.label')}
        description={t('settings.general.autoRemediation.project.helper')}
      >
        <SettingsSelect
          value={value.project_id ?? SOURCE_PROJECT}
          options={projectOptions}
          onChange={(projectId) =>
            update({
              project_id: projectId === SOURCE_PROJECT ? null : projectId,
            })
          }
        />
      </SettingsField>

      <SettingsField
        label={t('settings.general.autoRemediation.repos.label')}
        description={t('settings.general.autoRemediation.repos.helper')}
      >
        <div className="flex flex-col gap-half">
          {repos.map((repo) => (
            <SettingsCheckbox
              key={repo.id}
              id={`auto-error-remediation-repo-${repo.id}`}
              label={repo.display_name || repo.name}
              checked={value.repo_ids.includes(repo.id)}
              onChange={(checked) => toggleRepo(repo.id, checked)}
            />
          ))}
        </div>
      </SettingsField>

      <div className="grid grid-cols-2 gap-2">
        <SettingsField
          label={t('settings.general.autoRemediation.executor.label')}
        >
          <SettingsSelect
            value={value.executor}
            options={executors.map((executor) => ({
              value: executor,
              label: toPrettyCase(executor),
            }))}
            onChange={(executor) =>
              update({ executor: executor as BaseCodingAgent })
            }
          />
        </SettingsField>
        <SettingsField
          label={t('settings.general.autoRemediation.variant.label')}
          description={t('settings.general.autoRemediation.variant.helper')}
        >
          <SettingsInput
            value={value.variant ?? ''}
            onChange={(variant) => update({ variant: variant.trim() || null })}
            placeholder="DEFAULT"
          />
        </SettingsField>
        <SettingsField
          label={t('settings.general.autoRemediation.model.label')}
        >
          <SettingsInput
            value={value.model_id ?? ''}
            onChange={(model) => update({ model_id: model.trim() || null })}
            placeholder="claude-opus-5-5"
          />
        </SettingsField>
        <SettingsField
          label={t('settings.general.autoRemediation.maxPerHour.label')}
          description={t('settings.general.autoRemediation.maxPerHour.helper')}
        >
          <SettingsInput
            value={String(value.max_per_hour)}
            onChange={(raw) => {
              const parsed = Number.parseInt(raw, 10);
              update({
                max_per_hour:
                  Number.isFinite(parsed) && parsed > 0 ? parsed : 0,
              });
            }}
          />
        </SettingsField>
      </div>
    </SettingsCard>
  );
}
