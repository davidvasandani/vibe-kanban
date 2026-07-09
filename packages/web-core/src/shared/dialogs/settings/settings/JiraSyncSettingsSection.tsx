import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  SpinnerIcon,
  SignInIcon,
  PlusIcon,
  XIcon,
  ArrowsClockwiseIcon,
  TrashIcon,
} from '@phosphor-icons/react';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { Switch } from '@vibe/ui/components/Switch';
import { useUserOrganizations } from '@/shared/hooks/useUserOrganizations';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { OAuthDialog } from '@/shared/dialogs/global/OAuthDialog';
import { useShape } from '@/shared/integrations/electric/hooks';
import { PROJECTS_SHAPE } from 'shared/remote-types';
import type {
  JiraAuthMode,
  JiraStatusMapping,
  JiraTestConnectionResponse,
} from 'shared/remote-types';
import { cn } from '@/shared/lib/utils';
import {
  useJiraSyncConfig,
  useJiraSyncMutations,
} from '@/shared/hooks/useJiraSync';
import {
  SettingsCard,
  SettingsField,
  SettingsInput,
  SettingsSaveBar,
  SettingsSelect,
  SettingsTextarea,
  TwoColumnPicker,
  TwoColumnPickerColumn,
  TwoColumnPickerItem,
  TwoColumnPickerEmpty,
} from './SettingsComponents';
import { useSettingsDirty } from './SettingsDirtyContext';

interface JiraSyncSettingsSectionProps {
  initialState?: { organizationId?: string; projectId?: string };
}

interface FormState {
  jiraBaseUrl: string;
  authMode: JiraAuthMode;
  jiraEmail: string;
  /** Empty string means "keep the stored credential". */
  credential: string;
  jql: string;
  enabled: boolean;
  syncIntervalMinutes: number;
  statusMapping: JiraStatusMapping;
}

const EMPTY_FORM: FormState = {
  jiraBaseUrl: '',
  authMode: 'cloud_basic',
  jiraEmail: '',
  credential: '',
  jql: '',
  enabled: false,
  syncIntervalMinutes: 5,
  statusMapping: { jira_to_vk: {}, vk_to_jira: {} },
};

/** Password-masked variant of SettingsInput (which hardcodes type="text"). */
function SecretInput({
  value,
  onChange,
  placeholder,
  disabled,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
}) {
  return (
    <input
      type="password"
      autoComplete="off"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
      className={cn(
        'w-full bg-secondary border border-border rounded-sm px-base py-half text-sm text-high',
        'placeholder:text-low placeholder:opacity-80 focus:outline-none focus:ring-1 focus:ring-brand',
        disabled && 'opacity-50 cursor-not-allowed'
      )}
    />
  );
}

/** Editable name -> name mapping rows (used for both mapping directions). */
function MappingEditor({
  entries,
  keyPlaceholder,
  valuePlaceholder,
  addLabel,
  disabled,
  onChange,
}: {
  entries: Record<string, string>;
  keyPlaceholder: string;
  valuePlaceholder: string;
  addLabel: string;
  disabled?: boolean;
  onChange: (entries: Record<string, string>) => void;
}) {
  const rows = Object.entries(entries);
  return (
    <div className="space-y-half">
      {rows.map(([key, value], index) => (
        <div key={index} className="flex items-center gap-base">
          <input
            type="text"
            value={key}
            placeholder={keyPlaceholder}
            disabled={disabled}
            onChange={(e) => {
              const next = rows.slice();
              next[index] = [e.target.value, value];
              onChange(Object.fromEntries(next));
            }}
            className="flex-1 bg-secondary border border-border rounded-sm px-base py-half text-sm text-high focus:outline-none focus:ring-1 focus:ring-brand"
          />
          <span className="text-low text-sm shrink-0">→</span>
          <input
            type="text"
            value={value}
            placeholder={valuePlaceholder}
            disabled={disabled}
            onChange={(e) => {
              const next = rows.slice();
              next[index] = [key, e.target.value];
              onChange(Object.fromEntries(next));
            }}
            className="flex-1 bg-secondary border border-border rounded-sm px-base py-half text-sm text-high focus:outline-none focus:ring-1 focus:ring-brand"
          />
          <button
            type="button"
            disabled={disabled}
            onClick={() => {
              const next = rows.filter((_, i) => i !== index);
              onChange(Object.fromEntries(next));
            }}
            className="flex items-center justify-center size-icon-sm text-low hover:text-normal"
          >
            <XIcon className="size-icon-xs" weight="bold" />
          </button>
        </div>
      ))}
      <button
        type="button"
        disabled={disabled}
        onClick={() => onChange({ ...entries, '': '' })}
        className="flex items-center gap-half px-base py-half text-high hover:bg-secondary rounded-sm transition-colors"
      >
        <PlusIcon className="size-icon-xs" weight="bold" />
        <span className="text-xs font-light">{addLabel}</span>
      </button>
    </div>
  );
}

export function JiraSyncSettingsSection({
  initialState,
}: JiraSyncSettingsSectionProps) {
  const { t } = useTranslation(['settings', 'common']);
  const { setDirty: setContextDirty } = useSettingsDirty();
  const { isSignedIn, isLoaded } = useAuth();

  const [selectedOrgId, setSelectedOrgId] = useState<string | null>(
    initialState?.organizationId ?? null
  );
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    initialState?.projectId ?? null
  );

  const [form, setForm] = useState<FormState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] =
    useState<JiraTestConnectionResponse | null>(null);

  const { data: orgsResponse, isLoading: orgsLoading } =
    useUserOrganizations();
  const organizations = useMemo(
    () => orgsResponse?.organizations ?? [],
    [orgsResponse?.organizations]
  );

  useEffect(() => {
    if (
      !initialState?.organizationId &&
      organizations.length > 0 &&
      !selectedOrgId
    ) {
      setSelectedOrgId(organizations[0].id);
    }
  }, [organizations, selectedOrgId, initialState?.organizationId]);

  const params = useMemo(
    () => ({ organization_id: selectedOrgId || '' }),
    [selectedOrgId]
  );
  const { data: projects, isLoading: projectsLoading } = useShape(
    PROJECTS_SHAPE,
    params,
    { enabled: !!selectedOrgId }
  );

  const { data: config, isLoading: configLoading } = useJiraSyncConfig({
    projectId: selectedProjectId,
  });
  const { saveConfig, deleteConfig, testConnection, syncNow } =
    useJiraSyncMutations(selectedProjectId);

  // (Re)build the form from the loaded config whenever project or server
  // state changes and there are no local edits in flight.
  const [dirty, setDirty] = useState(false);
  useEffect(() => {
    if (dirty) return;
    if (!selectedProjectId) {
      setForm(null);
      return;
    }
    if (configLoading) return;
    if (!config) {
      setForm(EMPTY_FORM);
      return;
    }
    setForm({
      jiraBaseUrl: config.jira_base_url,
      authMode: config.auth_mode,
      jiraEmail: config.jira_email ?? '',
      credential: '',
      jql: config.jql,
      enabled: config.enabled,
      syncIntervalMinutes: Math.round(config.sync_interval_seconds / 60),
      statusMapping: config.status_mapping,
    });
  }, [selectedProjectId, config, configLoading, dirty]);

  useEffect(() => {
    setContextDirty('jira-sync', dirty);
    return () => setContextDirty('jira-sync', false);
  }, [dirty, setContextDirty]);

  const updateForm = (patch: Partial<FormState>) => {
    setForm((f) => (f ? { ...f, ...patch } : f));
    setDirty(true);
  };

  const handleProjectSelect = (projectId: string) => {
    if (dirty) {
      const confirmed = window.confirm(
        t('settings.common.discardChangesConfirm', 'Discard unsaved changes?')
      );
      if (!confirmed) return;
    }
    setSelectedProjectId(projectId);
    setDirty(false);
    setForm(null);
    setError(null);
    setTestResult(null);
  };

  const buildRequestMapping = (mapping: JiraStatusMapping) => ({
    jira_to_vk: Object.fromEntries(
      Object.entries(mapping.jira_to_vk).filter(([k, v]) => k.trim() && v.trim())
    ),
    vk_to_jira: Object.fromEntries(
      Object.entries(mapping.vk_to_jira).filter(([k, v]) => k.trim() && v.trim())
    ),
  });

  const handleSave = async () => {
    if (!form || !selectedProjectId) return;
    setError(null);
    try {
      await saveConfig.mutateAsync({
        jira_base_url: form.jiraBaseUrl.trim(),
        auth_mode: form.authMode,
        jira_email: form.jiraEmail.trim() || null,
        credential: form.credential.trim() || null,
        jql: form.jql.trim(),
        enabled: form.enabled,
        sync_interval_seconds: Math.min(
          3600,
          Math.max(60, Math.round(form.syncIntervalMinutes * 60))
        ),
        status_mapping: buildRequestMapping(form.statusMapping),
      });
      setDirty(false);
      setForm(null); // rebuilt from the refetched config
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t('settings.jiraSync.saveError', 'Failed to save Jira sync config')
      );
    }
  };

  const handleTest = async () => {
    if (!form) return;
    setError(null);
    setTestResult(null);
    try {
      const result = await testConnection.mutateAsync({
        jira_base_url: form.jiraBaseUrl.trim(),
        auth_mode: form.authMode,
        jira_email: form.jiraEmail.trim() || null,
        credential: form.credential.trim() || null,
        jql: form.jql.trim(),
      });
      setTestResult(result);
    } catch (err) {
      setTestResult({
        ok: false,
        match_count: null,
        jira_statuses: [],
        error:
          err instanceof Error
            ? err.message
            : t('settings.jiraSync.testError', 'Connection test failed'),
      });
    }
  };

  const handleDelete = async () => {
    if (!selectedProjectId) return;
    const confirmed = window.confirm(
      t(
        'settings.jiraSync.deleteConfirm',
        'Disconnect Jira? Synced tasks stay on the board but stop syncing.'
      )
    );
    if (!confirmed) return;
    setError(null);
    try {
      await deleteConfig.mutateAsync();
      setDirty(false);
      setForm(null);
      setTestResult(null);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t('settings.jiraSync.deleteError', 'Failed to disconnect Jira')
      );
    }
  };

  const syncRunning =
    !!config?.last_sync_started_at &&
    (!config.last_sync_completed_at ||
      config.last_sync_started_at > config.last_sync_completed_at);

  if (!isLoaded || orgsLoading) {
    return (
      <div className="flex items-center justify-center py-8 gap-2">
        <SpinnerIcon
          className="size-icon-lg animate-spin text-brand"
          weight="bold"
        />
        <span className="text-normal">
          {t('settings.jiraSync.loading', 'Loading Jira sync settings...')}
        </span>
      </div>
    );
  }

  if (!isSignedIn) {
    return (
      <div className="space-y-4">
        <div>
          <h3 className="text-base font-medium text-high">
            {t('settings.jiraSync.loginRequired.title', 'Sign in required')}
          </h3>
          <p className="text-sm text-low mt-1">
            {t(
              'settings.jiraSync.loginRequired.description',
              'Sign in to configure Jira sync for your projects.'
            )}
          </p>
        </div>
        <PrimaryButton
          variant="secondary"
          value={t('settings.jiraSync.loginRequired.action', 'Sign in')}
          onClick={() => void OAuthDialog.show({})}
        >
          <SignInIcon className="size-icon-xs mr-1" weight="bold" />
        </PrimaryButton>
      </div>
    );
  }

  const saving = saveConfig.isPending;

  return (
    <>
      {error && (
        <div className="bg-error/10 border border-error/50 rounded-sm p-4 text-error mb-4">
          {error}
        </div>
      )}

      <SettingsCard
        title={t('settings.jiraSync.title', 'Jira Sync')}
        description={t(
          'settings.jiraSync.description',
          'Mirror Jira issues matching a JQL query onto a project board, and push board changes back to Jira.'
        )}
      >
        <TwoColumnPicker>
          <TwoColumnPickerColumn
            label={t('settings.jiraSync.columns.organizations', 'Organizations')}
            isFirst
          >
            {organizations.map((org) => (
              <TwoColumnPickerItem
                key={org.id}
                selected={selectedOrgId === org.id}
                onClick={() => {
                  setSelectedOrgId(org.id);
                  setSelectedProjectId(null);
                  setForm(null);
                  setDirty(false);
                }}
              >
                {org.name}
              </TwoColumnPickerItem>
            ))}
          </TwoColumnPickerColumn>
          <TwoColumnPickerColumn
            label={t('settings.jiraSync.columns.projects', 'Projects')}
          >
            {projectsLoading ? (
              <div className="flex items-center justify-center py-double gap-base">
                <SpinnerIcon className="size-icon-sm animate-spin" />
              </div>
            ) : selectedOrgId && projects.length > 0 ? (
              projects.map((project) => (
                <TwoColumnPickerItem
                  key={project.id}
                  selected={selectedProjectId === project.id}
                  onClick={() => handleProjectSelect(project.id)}
                  leading={
                    <span
                      className="w-3 h-3 rounded-full shrink-0"
                      style={{ backgroundColor: `hsl(${project.color})` }}
                    />
                  }
                >
                  {project.name}
                </TwoColumnPickerItem>
              ))
            ) : (
              <TwoColumnPickerEmpty>
                {t('settings.jiraSync.selectOrg', 'Select an organization')}
              </TwoColumnPickerEmpty>
            )}
          </TwoColumnPickerColumn>
        </TwoColumnPicker>

        {selectedProjectId && configLoading && !form && (
          <div className="flex items-center justify-center py-double gap-base">
            <SpinnerIcon className="size-icon-sm animate-spin" />
          </div>
        )}

        {selectedProjectId && form && (
          <>
            {/* Connection */}
            <div className="bg-secondary/50 border border-border rounded-sm p-4 space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-normal">
                    {t('settings.jiraSync.form.enabled.label', 'Sync enabled')}
                  </p>
                  <p className="text-sm text-low mt-1">
                    {t(
                      'settings.jiraSync.form.enabled.description',
                      'While enabled, the server reconciles Jira and the board on the configured interval.'
                    )}
                  </p>
                </div>
                <Switch
                  checked={form.enabled}
                  onCheckedChange={(enabled) => updateForm({ enabled })}
                  disabled={saving}
                />
              </div>

              <SettingsField
                label={t('settings.jiraSync.form.baseUrl.label', 'Jira URL')}
              >
                <SettingsInput
                  value={form.jiraBaseUrl}
                  onChange={(jiraBaseUrl) => updateForm({ jiraBaseUrl })}
                  placeholder="https://your-team.atlassian.net"
                  disabled={saving}
                />
              </SettingsField>

              <SettingsField
                label={t(
                  'settings.jiraSync.form.authMode.label',
                  'Authentication'
                )}
              >
                <SettingsSelect<JiraAuthMode>
                  value={form.authMode}
                  options={[
                    {
                      value: 'cloud_basic',
                      label: t(
                        'settings.jiraSync.form.authMode.cloudBasic',
                        'Jira Cloud (email + API token)'
                      ),
                    },
                    {
                      value: 'server_pat',
                      label: t(
                        'settings.jiraSync.form.authMode.serverPat',
                        'Jira Server / Data Center (personal access token)'
                      ),
                    },
                  ]}
                  onChange={(authMode) => updateForm({ authMode })}
                  disabled={saving}
                />
              </SettingsField>

              {form.authMode === 'cloud_basic' && (
                <SettingsField
                  label={t('settings.jiraSync.form.email.label', 'Email')}
                >
                  <SettingsInput
                    value={form.jiraEmail}
                    onChange={(jiraEmail) => updateForm({ jiraEmail })}
                    placeholder="you@example.com"
                    disabled={saving}
                  />
                </SettingsField>
              )}

              <SettingsField
                label={
                  form.authMode === 'cloud_basic'
                    ? t('settings.jiraSync.form.credential.label', 'API token')
                    : t(
                        'settings.jiraSync.form.credential.labelPat',
                        'Personal access token'
                      )
                }
                description={
                  config?.has_credential
                    ? t(
                        'settings.jiraSync.form.credential.stored',
                        'A credential is stored. Leave blank to keep it.'
                      )
                    : undefined
                }
              >
                <SecretInput
                  value={form.credential}
                  onChange={(credential) => updateForm({ credential })}
                  placeholder={
                    config?.has_credential ? '••••••••' : undefined
                  }
                  disabled={saving}
                />
              </SettingsField>

              <SettingsField
                label={t('settings.jiraSync.form.jql.label', 'JQL query')}
                description={t(
                  'settings.jiraSync.form.jql.description',
                  'Issues matching this query appear as tasks on the board.'
                )}
              >
                <SettingsTextarea
                  value={form.jql}
                  onChange={(jql) => updateForm({ jql })}
                  placeholder='project = ABC AND labels = "vk-sync" ORDER BY created DESC'
                  rows={3}
                  monospace
                  disabled={saving}
                />
              </SettingsField>

              <SettingsField
                label={t(
                  'settings.jiraSync.form.interval.label',
                  'Sync interval (minutes)'
                )}
              >
                <SettingsInput
                  value={String(form.syncIntervalMinutes)}
                  onChange={(v) => {
                    const parsed = Number(v);
                    updateForm({
                      syncIntervalMinutes: Number.isFinite(parsed)
                        ? parsed
                        : form.syncIntervalMinutes,
                    });
                  }}
                  disabled={saving}
                />
              </SettingsField>

              <div className="flex items-center gap-base">
                <PrimaryButton
                  variant="tertiary"
                  value={t(
                    'settings.jiraSync.actions.test',
                    'Test connection'
                  )}
                  actionIcon={
                    testConnection.isPending ? 'spinner' : undefined
                  }
                  onClick={() => void handleTest()}
                  disabled={
                    testConnection.isPending ||
                    !form.jiraBaseUrl.trim() ||
                    !form.jql.trim()
                  }
                />
                {testResult &&
                  (testResult.ok ? (
                    <span className="text-sm text-success">
                      {t('settings.jiraSync.testOk', 'Connection OK')}
                      {testResult.match_count !== null &&
                        ` — ${t('settings.jiraSync.testMatches', '{{count}} matching issues', { count: Number(testResult.match_count) })}`}
                    </span>
                  ) : (
                    <span className="text-sm text-error">
                      {testResult.error}
                    </span>
                  ))}
              </div>
            </div>

            {/* Status mapping */}
            <div className="bg-secondary/50 border border-border rounded-sm p-4 space-y-4">
              <div>
                <p className="text-sm font-medium text-normal">
                  {t(
                    'settings.jiraSync.form.mapping.label',
                    'Status mapping'
                  )}
                </p>
                <p className="text-sm text-low mt-1">
                  {t(
                    'settings.jiraSync.form.mapping.description',
                    'Unmapped Jira statuses fall back to their status category (To Do → "To do", In Progress → "In progress", Done → "Done"). Board-to-Jira entries are seeded automatically on the first sync.'
                  )}
                </p>
              </div>

              <SettingsField
                label={t(
                  'settings.jiraSync.form.mapping.jiraToVk',
                  'Jira status → board column'
                )}
              >
                <MappingEditor
                  entries={form.statusMapping.jira_to_vk}
                  keyPlaceholder={t(
                    'settings.jiraSync.form.mapping.jiraStatus',
                    'Jira status'
                  )}
                  valuePlaceholder={t(
                    'settings.jiraSync.form.mapping.vkStatus',
                    'Board column'
                  )}
                  addLabel={t(
                    'settings.jiraSync.form.mapping.addOverride',
                    'Add override'
                  )}
                  disabled={saving}
                  onChange={(jira_to_vk) =>
                    updateForm({
                      statusMapping: { ...form.statusMapping, jira_to_vk },
                    })
                  }
                />
              </SettingsField>

              <SettingsField
                label={t(
                  'settings.jiraSync.form.mapping.vkToJira',
                  'Board column → Jira status'
                )}
              >
                <MappingEditor
                  entries={form.statusMapping.vk_to_jira}
                  keyPlaceholder={t(
                    'settings.jiraSync.form.mapping.vkStatus',
                    'Board column'
                  )}
                  valuePlaceholder={t(
                    'settings.jiraSync.form.mapping.jiraStatus',
                    'Jira status'
                  )}
                  addLabel={t(
                    'settings.jiraSync.form.mapping.addMapping',
                    'Add mapping'
                  )}
                  disabled={saving}
                  onChange={(vk_to_jira) =>
                    updateForm({
                      statusMapping: { ...form.statusMapping, vk_to_jira },
                    })
                  }
                />
              </SettingsField>
            </div>

            {/* Sync status */}
            {config && (
              <div className="bg-secondary/50 border border-border rounded-sm p-4 space-y-base">
                <div className="flex items-center justify-between">
                  <p className="text-sm font-medium text-normal">
                    {t('settings.jiraSync.status.label', 'Sync status')}
                  </p>
                  <div className="flex items-center gap-base">
                    <PrimaryButton
                      variant="tertiary"
                      value={t('settings.jiraSync.actions.syncNow', 'Sync now')}
                      actionIcon={
                        syncNow.isPending || syncRunning
                          ? 'spinner'
                          : ArrowsClockwiseIcon
                      }
                      onClick={() => void syncNow.mutateAsync().catch(() => {})}
                      disabled={
                        syncNow.isPending || !config.enabled || dirty
                      }
                    />
                    <PrimaryButton
                      variant="tertiary"
                      value={t(
                        'settings.jiraSync.actions.disconnect',
                        'Disconnect'
                      )}
                      actionIcon={
                        deleteConfig.isPending ? 'spinner' : TrashIcon
                      }
                      onClick={() => void handleDelete()}
                      disabled={deleteConfig.isPending}
                    />
                  </div>
                </div>

                <div className="text-sm text-low space-y-half">
                  <p>
                    {syncRunning
                      ? t('settings.jiraSync.status.running', 'Sync running…')
                      : config.last_sync_completed_at
                        ? t('settings.jiraSync.status.lastSync', {
                            defaultValue: 'Last synced: {{time}}',
                            time: new Date(
                              config.last_sync_completed_at
                            ).toLocaleString(),
                          })
                        : t(
                            'settings.jiraSync.status.never',
                            'Not synced yet'
                          )}
                  </p>
                  <p>
                    {t('settings.jiraSync.status.links', {
                      defaultValue:
                        '{{active}} linked · {{dormant}} out of scope · {{deleted}} deleted in Jira · {{errored}} with errors',
                      active: Number(config.link_counts.active),
                      dormant: Number(config.link_counts.dormant),
                      deleted: Number(config.link_counts.deleted_remote),
                      errored: Number(config.link_counts.errored),
                    })}
                  </p>
                  {config.last_sync_error && (
                    <p className="text-error">{config.last_sync_error}</p>
                  )}
                </div>
              </div>
            )}
          </>
        )}
      </SettingsCard>

      <SettingsSaveBar
        show={dirty}
        saving={saving}
        onSave={() => void handleSave()}
        onDiscard={() => {
          setDirty(false);
          setForm(null);
          setTestResult(null);
        }}
      />
    </>
  );
}
