import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  SpinnerIcon,
  SignInIcon,
  TrashIcon,
  CopyIcon,
  CheckIcon,
} from '@phosphor-icons/react';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { Switch } from '@vibe/ui/components/Switch';
import { useUserOrganizations } from '@/shared/hooks/useUserOrganizations';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { OAuthDialog } from '@/shared/dialogs/global/OAuthDialog';
import type { SlackTestConnectionResponse } from 'shared/remote-types';
import { cn } from '@/shared/lib/utils';
import { getRemoteApiUrl } from '@/shared/lib/remoteApi';
import {
  useSlackConfig,
  useSlackMutations,
} from '@/shared/hooks/useSlackIntegration';
import {
  SettingsCard,
  SettingsField,
  SettingsSaveBar,
  SettingsSelect,
} from './SettingsComponents';
import { useSettingsDirty } from './SettingsDirtyContext';

interface SlackSettingsSectionProps {
  initialState?: { organizationId?: string };
}

interface FormState {
  /** Empty string means "keep the stored credential". */
  botToken: string;
  /** Empty string means "keep the stored credential". */
  signingSecret: string;
  enabled: boolean;
}

const EMPTY_FORM: FormState = {
  botToken: '',
  signingSecret: '',
  enabled: true,
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

function buildManifest(requestUrl: string): string {
  return `display_information:
  name: Vibe Kanban
  description: Create Vibe Kanban issues from Slack messages
features:
  bot_user:
    display_name: Vibe Kanban
    always_online: true
  shortcuts:
    - name: Create issue from message
      type: message
      callback_id: vk_create_issue_from_message
      description: Create a Vibe Kanban issue from this message
oauth_config:
  scopes:
    bot:
      - commands
      - chat:write
      - im:write
settings:
  interactivity:
    is_enabled: true
    request_url: ${requestUrl}
  org_deploy_enabled: false
  socket_mode_enabled: false`;
}

export function SlackSettingsSection({
  initialState,
}: SlackSettingsSectionProps) {
  const { t } = useTranslation(['settings', 'common']);
  const { setDirty: setContextDirty } = useSettingsDirty();
  const { isSignedIn, isLoaded } = useAuth();

  const [selectedOrgId, setSelectedOrgId] = useState<string | null>(
    initialState?.organizationId ?? null
  );
  const [form, setForm] = useState<FormState | null>(null);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] =
    useState<SlackTestConnectionResponse | null>(null);
  const [manifestCopied, setManifestCopied] = useState(false);

  const { data: orgsResponse, isLoading: orgsLoading } = useUserOrganizations();
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

  const { data: config, isLoading: configLoading } = useSlackConfig({
    organizationId: selectedOrgId,
  });
  const { saveConfig, deleteConfig, testConnection } =
    useSlackMutations(selectedOrgId);

  // (Re)build the form from the loaded config whenever org or server state
  // changes and there are no local edits in flight.
  useEffect(() => {
    if (dirty) return;
    if (!selectedOrgId) {
      setForm(null);
      return;
    }
    if (configLoading) return;
    setForm({
      botToken: '',
      signingSecret: '',
      enabled: config?.enabled ?? EMPTY_FORM.enabled,
    });
  }, [selectedOrgId, config, configLoading, dirty]);

  useEffect(() => {
    setContextDirty('slack', dirty);
    return () => setContextDirty('slack', false);
  }, [dirty, setContextDirty]);

  const updateForm = (patch: Partial<FormState>) => {
    setForm((f) => (f ? { ...f, ...patch } : f));
    setDirty(true);
  };

  const interactivityUrl =
    config?.interactivity_url ??
    `${getRemoteApiUrl().replace(/\/$/, '')}/v1/slack/interactivity`;
  const manifest = buildManifest(interactivityUrl);

  const handleSave = async () => {
    if (!form || !selectedOrgId) return;
    setError(null);
    try {
      await saveConfig.mutateAsync({
        bot_token: form.botToken.trim() || null,
        signing_secret: form.signingSecret.trim() || null,
        enabled: form.enabled,
      });
      setDirty(false);
      setForm(null); // rebuilt from the refetched config
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t('settings.slack.saveError', 'Failed to save Slack settings')
      );
    }
  };

  const handleTest = async () => {
    setError(null);
    setTestResult(null);
    try {
      const result = await testConnection.mutateAsync();
      setTestResult(result);
    } catch (err) {
      setTestResult({
        ok: false,
        team_name: null,
        error:
          err instanceof Error
            ? err.message
            : t('settings.slack.testError', 'Connection test failed'),
      });
    }
  };

  const handleDelete = async () => {
    if (!selectedOrgId) return;
    const confirmed = window.confirm(
      t(
        'settings.slack.deleteConfirm',
        'Disconnect Slack? Issues created from Slack stay on the board; the shortcut stops working.'
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
          : t('settings.slack.deleteError', 'Failed to disconnect Slack')
      );
    }
  };

  const handleCopyManifest = async () => {
    try {
      await navigator.clipboard.writeText(manifest);
      setManifestCopied(true);
      setTimeout(() => setManifestCopied(false), 2000);
    } catch {
      // Clipboard unavailable (permissions/insecure context) — the manifest
      // is still selectable text.
    }
  };

  if (!isLoaded || orgsLoading) {
    return (
      <div className="flex items-center justify-center py-8 gap-2">
        <SpinnerIcon
          className="size-icon-lg animate-spin text-brand"
          weight="bold"
        />
        <span className="text-normal">
          {t('settings.slack.loading', 'Loading Slack settings...')}
        </span>
      </div>
    );
  }

  if (!isSignedIn) {
    return (
      <div className="space-y-4">
        <div>
          <h3 className="text-base font-medium text-high">
            {t('settings.slack.loginRequired.title', 'Sign in required')}
          </h3>
          <p className="text-sm text-low mt-1">
            {t(
              'settings.slack.loginRequired.description',
              'Sign in to connect Slack to your organization.'
            )}
          </p>
        </div>
        <PrimaryButton
          variant="secondary"
          value={t('settings.slack.loginRequired.action', 'Sign in')}
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
        title={t('settings.slack.title', 'Slack')}
        description={t(
          'settings.slack.description',
          'Connect a Slack workspace so anyone can create issues from Slack messages with the "Create issue from message" shortcut.'
        )}
      >
        <SettingsField
          label={t('settings.slack.form.organization.label', 'Organization')}
        >
          <SettingsSelect<string>
            value={selectedOrgId ?? ''}
            options={organizations.map((org) => ({
              value: org.id,
              label: org.name,
            }))}
            onChange={(orgId) => {
              if (dirty) {
                const confirmed = window.confirm(
                  t(
                    'settings.common.discardChangesConfirm',
                    'Discard unsaved changes?'
                  )
                );
                if (!confirmed) return;
              }
              setSelectedOrgId(orgId);
              setDirty(false);
              setForm(null);
              setError(null);
              setTestResult(null);
            }}
            disabled={saving}
          />
        </SettingsField>

        {selectedOrgId && configLoading && !form && (
          <div className="flex items-center justify-center py-double gap-base">
            <SpinnerIcon className="size-icon-sm animate-spin" />
          </div>
        )}

        {selectedOrgId && form && (
          <>
            {/* Connection */}
            <div className="bg-secondary/50 border border-border rounded-sm p-4 space-y-4">
              {config && (
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-sm font-medium text-normal">
                      {t(
                        'settings.slack.form.workspace.label',
                        'Connected workspace'
                      )}
                    </p>
                    <p className="text-sm text-low mt-1">
                      {config.slack_team_name}
                      {config.slack_team_id && ` (${config.slack_team_id})`}
                    </p>
                  </div>
                  <PrimaryButton
                    variant="tertiary"
                    value={t('settings.slack.actions.disconnect', 'Disconnect')}
                    actionIcon={deleteConfig.isPending ? 'spinner' : TrashIcon}
                    onClick={() => void handleDelete()}
                    disabled={deleteConfig.isPending}
                  />
                </div>
              )}

              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-normal">
                    {t('settings.slack.form.enabled.label', 'Shortcut enabled')}
                  </p>
                  <p className="text-sm text-low mt-1">
                    {t(
                      'settings.slack.form.enabled.description',
                      'While enabled, the message shortcut creates issues in this organization.'
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
                label={t('settings.slack.form.botToken.label', 'Bot token')}
                description={
                  config?.has_credentials
                    ? t(
                        'settings.slack.form.credential.stored',
                        'A credential is stored. Leave blank to keep it.'
                      )
                    : t(
                        'settings.slack.form.botToken.description',
                        'The xoxb- token from your Slack app’s "OAuth & Permissions" page.'
                      )
                }
              >
                <SecretInput
                  value={form.botToken}
                  onChange={(botToken) => updateForm({ botToken })}
                  placeholder={
                    config?.has_credentials ? '••••••••' : 'xoxb-...'
                  }
                  disabled={saving}
                />
              </SettingsField>

              <SettingsField
                label={t(
                  'settings.slack.form.signingSecret.label',
                  'Signing secret'
                )}
                description={
                  config?.has_credentials
                    ? t(
                        'settings.slack.form.credential.stored',
                        'A credential is stored. Leave blank to keep it.'
                      )
                    : t(
                        'settings.slack.form.signingSecret.description',
                        'From your Slack app’s "Basic Information" page; used to verify requests from Slack.'
                      )
                }
              >
                <SecretInput
                  value={form.signingSecret}
                  onChange={(signingSecret) => updateForm({ signingSecret })}
                  placeholder={config?.has_credentials ? '••••••••' : undefined}
                  disabled={saving}
                />
              </SettingsField>

              {config && (
                <div className="flex items-center gap-base">
                  <PrimaryButton
                    variant="tertiary"
                    value={t('settings.slack.actions.test', 'Test connection')}
                    actionIcon={
                      testConnection.isPending ? 'spinner' : undefined
                    }
                    onClick={() => void handleTest()}
                    disabled={testConnection.isPending}
                  />
                  {testResult &&
                    (testResult.ok ? (
                      <span className="text-sm text-success">
                        {t('settings.slack.testOk', {
                          defaultValue: 'Connected to {{team}}',
                          team: testResult.team_name ?? 'Slack',
                        })}
                      </span>
                    ) : (
                      <span className="text-sm text-error">
                        {testResult.error}
                      </span>
                    ))}
                </div>
              )}
            </div>

            {/* App manifest */}
            <div className="bg-secondary/50 border border-border rounded-sm p-4 space-y-base">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-normal">
                    {t('settings.slack.manifest.label', 'Slack app manifest')}
                  </p>
                  <p className="text-sm text-low mt-1">
                    {t(
                      'settings.slack.manifest.description',
                      'Create a Slack app from this manifest (api.slack.com/apps → "Create New App" → "From a manifest"), install it to your workspace, then paste the bot token and signing secret above.'
                    )}
                  </p>
                </div>
                <PrimaryButton
                  variant="tertiary"
                  value={
                    manifestCopied
                      ? t('settings.slack.manifest.copied', 'Copied')
                      : t('settings.slack.manifest.copy', 'Copy')
                  }
                  actionIcon={manifestCopied ? CheckIcon : CopyIcon}
                  onClick={() => void handleCopyManifest()}
                />
              </div>
              <pre className="text-xs text-low bg-secondary border border-border rounded-sm p-base overflow-x-auto whitespace-pre">
                {manifest}
              </pre>
            </div>
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
