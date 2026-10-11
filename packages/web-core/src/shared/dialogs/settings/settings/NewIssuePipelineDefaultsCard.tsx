import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import type { NewIssuePipelineDefaults } from 'shared/types';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { useUserOrganizations } from '@/shared/hooks/useUserOrganizations';
import { useAllOrganizationProjects } from '@/shared/hooks/useAllOrganizationProjects';
import {
  PIPELINES_QUERY_KEY,
  usePipelineStatuses,
} from '@/shared/hooks/usePipelines';
import {
  clearScopeOverride,
  getScopeOverride,
  setScopeOverride,
  toggleMachineDefault,
  toggleScopePipeline,
  type PipelineDefaultsScope,
} from '@/shared/lib/pipeline/pipelineSettings';
import {
  SettingsCard,
  SettingsCheckbox,
  SettingsSelect,
} from './SettingsComponents';
import { useSettingsMachineClient } from './SettingsHostContext';

// Used when the selected machine runs a server that predates the setting.
const FALLBACK_DEFAULTS: NewIssuePipelineDefaults = {
  pipeline_ids: ['basic'],
  organizations: {},
  projects: {},
};

type OverrideMode = 'inherit' | 'override';

interface PipelineOption {
  id: string;
  name: string;
}

/**
 * Checkboxes for every available pipeline, plus any selected id whose file
 * no longer loads (shown as missing so it can be unticked).
 */
function PipelineChecklist({
  idPrefix,
  options,
  selected,
  disabled,
  onToggle,
}: {
  idPrefix: string;
  options: readonly PipelineOption[];
  selected: readonly string[];
  disabled: boolean;
  onToggle: (pipelineId: string, enabled: boolean) => void;
}) {
  const { t } = useTranslation('settings');
  const missing = selected.filter((id) => !options.some((o) => o.id === id));
  return (
    <div className="flex flex-col gap-2">
      {options.map((option) => (
        <SettingsCheckbox
          key={option.id}
          id={`${idPrefix}-${option.id}`}
          label={option.name}
          checked={selected.includes(option.id)}
          disabled={disabled}
          onChange={(enabled) => onToggle(option.id, enabled)}
        />
      ))}
      {missing.map((id) => (
        <SettingsCheckbox
          key={id}
          id={`${idPrefix}-${id}`}
          label={t('settings.pipelines.newIssueDefaults.missing', { id })}
          checked
          disabled={disabled}
          onChange={(enabled) => onToggle(id, enabled)}
        />
      ))}
    </div>
  );
}

function ScopeOverrideRow({
  scope,
  scopeId,
  name,
  inheritLabel,
  defaults,
  options,
  disabled,
  onChange,
}: {
  scope: PipelineDefaultsScope;
  scopeId: string;
  name: string;
  inheritLabel: string;
  defaults: NewIssuePipelineDefaults;
  options: readonly PipelineOption[];
  disabled: boolean;
  onChange: (next: NewIssuePipelineDefaults) => void;
}) {
  const { t } = useTranslation('settings');
  const override = getScopeOverride(defaults, scope, scopeId);
  const order = options.map((o) => o.id);
  return (
    <div className="space-y-2 rounded-sm border border-border p-3">
      <div className="flex min-w-0 flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <span className="min-w-0 truncate text-sm font-medium text-normal">
          {name}
        </span>
        <SettingsSelect<OverrideMode>
          className="sm:w-56"
          value={override === undefined ? 'inherit' : 'override'}
          disabled={disabled}
          options={[
            { value: 'inherit', label: inheritLabel },
            {
              value: 'override',
              label: t('settings.pipelines.newIssueDefaults.override'),
            },
          ]}
          onChange={(mode) =>
            onChange(
              mode === 'inherit'
                ? clearScopeOverride(defaults, scope, scopeId)
                : setScopeOverride(defaults, scope, scopeId, [])
            )
          }
        />
      </div>
      {override !== undefined && (
        <>
          <PipelineChecklist
            idPrefix={`new-issue-pipelines-${scope}-${scopeId}`}
            options={options}
            selected={override}
            disabled={disabled}
            onToggle={(pipelineId, enabled) =>
              onChange(
                toggleScopePipeline(
                  defaults,
                  scope,
                  scopeId,
                  pipelineId,
                  enabled,
                  order
                )
              )
            }
          />
          {override.length === 0 && (
            <p className="text-xs text-low">
              {t('settings.pipelines.newIssueDefaults.noneSelected')}
            </p>
          )}
        </>
      )}
    </div>
  );
}

/**
 * Settings → Pipelines card choosing which pipelines new issues start with
 * (New Issue UI and MCP `create_issue` without `pipeline_ids`): a machine
 * default plus per-organization and per-project overrides. Stored in the
 * selected machine's config, beside the pipeline files the ids name. Each
 * change saves immediately and replaces the whole `new_issue_pipelines`
 * object, so removing an id never deep-merges back.
 */
export function NewIssuePipelineDefaultsCard() {
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();
  const machineClient = useSettingsMachineClient();
  const { config, updateAndSaveConfig } = useUserSystem();
  const { data: statuses = [] } = usePipelineStatuses(machineClient);
  const { data: orgsData } = useUserOrganizations();
  const { data: projects } = useAllOrganizationProjects();
  const [saving, setSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);

  const defaults = config?.new_issue_pipelines ?? FALLBACK_DEFAULTS;
  const options = useMemo<PipelineOption[]>(
    () =>
      statuses
        .filter((s) => s.valid)
        .map((s) => ({ id: s.id, name: s.name || s.id })),
    [statuses]
  );
  const order = options.map((o) => o.id);
  const organizations = orgsData?.organizations ?? [];

  const save = async (next: NewIssuePipelineDefaults) => {
    setSaving(true);
    setSaveFailed(false);
    const ok = await updateAndSaveConfig({ new_issue_pipelines: next });
    setSaving(false);
    setSaveFailed(!ok);
    void queryClient.invalidateQueries({ queryKey: PIPELINES_QUERY_KEY });
  };

  if (!config) return null;

  return (
    <SettingsCard
      title={t('settings.pipelines.newIssueDefaults.title')}
      description={t('settings.pipelines.newIssueDefaults.description')}
    >
      <div className="space-y-2">
        <h4 className="text-sm font-medium text-high">
          {t('settings.pipelines.newIssueDefaults.machine.label')}
        </h4>
        <p className="text-sm text-low">
          {t('settings.pipelines.newIssueDefaults.machine.helper')}
        </p>
        <PipelineChecklist
          idPrefix="new-issue-pipelines-machine"
          options={options}
          selected={defaults.pipeline_ids}
          disabled={saving}
          onToggle={(pipelineId, enabled) =>
            void save(
              toggleMachineDefault(defaults, pipelineId, enabled, order)
            )
          }
        />
      </div>

      {organizations.length > 0 && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-high">
            {t('settings.pipelines.newIssueDefaults.organizations.label')}
          </h4>
          <p className="text-sm text-low">
            {t('settings.pipelines.newIssueDefaults.organizations.helper')}
          </p>
          {organizations.map((org) => (
            <ScopeOverrideRow
              key={org.id}
              scope="organizations"
              scopeId={org.id}
              name={org.name}
              inheritLabel={t(
                'settings.pipelines.newIssueDefaults.inheritMachine'
              )}
              defaults={defaults}
              options={options}
              disabled={saving}
              onChange={(next) => void save(next)}
            />
          ))}
        </div>
      )}

      {projects.length > 0 && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-high">
            {t('settings.pipelines.newIssueDefaults.projects.label')}
          </h4>
          <p className="text-sm text-low">
            {t('settings.pipelines.newIssueDefaults.projects.helper')}
          </p>
          {projects.map((project) => {
            const org = organizations.find(
              (o) => o.id === project.organization_id
            );
            return (
              <ScopeOverrideRow
                key={project.id}
                scope="projects"
                scopeId={project.id}
                name={org ? `${org.name} / ${project.name}` : project.name}
                inheritLabel={t(
                  'settings.pipelines.newIssueDefaults.inheritOrganization'
                )}
                defaults={defaults}
                options={options}
                disabled={saving}
                onChange={(next) => void save(next)}
              />
            );
          })}
        </div>
      )}

      {saveFailed && (
        <p className="text-sm text-error">
          {t('settings.pipelines.newIssueDefaults.saveFailed')}
        </p>
      )}
    </SettingsCard>
  );
}
