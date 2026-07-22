import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ArrowsClockwiseIcon,
  CheckCircleIcon,
  PencilSimpleIcon,
  PlusIcon,
  SpinnerIcon,
  TrashIcon,
  WarningCircleIcon,
} from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import { Input } from '@vibe/ui/components/Input';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import type {
  AwsProfileImportResult,
  AwsSsoProfile,
  AwsSsoProfileStatus,
  AwsSsoSession,
} from 'shared/types';
import {
  canEditAwsProfile,
  getAwsProfileLoginAction,
} from '@/shared/lib/awsProfileActions';
import {
  buildAwsImportRequest,
  buildAwsImportRows,
  defaultAwsSessionName,
  isAwsImportBlocked,
  type AwsImportRow,
} from '@/shared/lib/awsProfileImport';
import { getTerminalTheme } from '@/shared/lib/terminalTheme';
import { SettingsCard, SettingsField } from './SettingsComponents';
import { useSettingsMachineClient } from './SettingsHostContext';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import '@xterm/xterm/css/xterm.css';

const NAME_PATTERN = /^[A-Za-z0-9_.@-]{1,128}$/;
const REGION_PATTERN = /^[a-z]{2}(-[a-z]+)+-\d+$/;
const ROLE_PATTERN = /^[A-Za-z0-9+=,.@_-]{1,64}$/;
const ACCOUNT_PATTERN = /^\d{12}$/;
const OUTPUTS = ['json', 'yaml', 'text', 'table'];

const EMPTY_FORM: AwsSsoProfile = {
  name: '',
  sso_start_url: '',
  sso_region: '',
  sso_account_id: '',
  sso_role_name: '',
  region: null,
  output: null,
};

type EditorState =
  | { mode: 'add' }
  | { mode: 'edit'; profile: AwsSsoProfile }
  | null;

export function AwsSettingsSection() {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const [profiles, setProfiles] = useState<AwsSsoProfileStatus[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [editor, setEditor] = useState<EditorState>(null);
  const [loginProfile, setLoginProfile] = useState<string | null>(null);
  const [importOpen, setImportOpen] = useState(false);

  const refresh = useCallback(async () => {
    if (!machineClient) return;
    setRefreshing(true);
    try {
      setProfiles(await machineClient.listAwsProfiles());
      setLoadError(null);
    } catch (err) {
      setLoadError(err instanceof Error ? err.message : String(err));
    } finally {
      setRefreshing(false);
    }
  }, [machineClient]);

  useEffect(() => {
    setProfiles(null);
    setLoadError(null);
    setEditor(null);
    setLoginProfile(null);
    void refresh();
  }, [refresh]);

  const cliMissing =
    profiles?.some((p) => p.auth.status === 'cli_missing') ?? false;

  const applyStatus = (updated: AwsSsoProfileStatus) => {
    setProfiles(
      (current) =>
        current?.map((item) =>
          item.profile.name === updated.profile.name ? updated : item
        ) ?? current
    );
  };

  const handleDelete = async (name: string) => {
    if (!machineClient) return;
    const confirmed = window.confirm(
      t('settings.aws.delete.confirm', { ns: 'settings', name })
    );
    if (!confirmed) return;
    setBusy(true);
    try {
      await machineClient.deleteAwsProfile(name);
      setLoadError(null);
      await refresh();
    } catch (err) {
      setLoadError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <SettingsCard
      title={t('settings.aws.title', { ns: 'settings' })}
      description={t('settings.aws.description', { ns: 'settings' })}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-sm text-low">
          {profiles === null && !loadError && (
            <>
              <SpinnerIcon className="size-icon-sm animate-spin" />
              {t('settings.aws.loading', { ns: 'settings' })}
            </>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Button
            size="sm"
            variant="secondary"
            disabled={busy || editor !== null || importOpen}
            onClick={() => setImportOpen(true)}
          >
            {t('settings.aws.import.action', { ns: 'settings' })}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={refreshing || !machineClient}
            onClick={() => void refresh()}
            aria-label={t('settings.aws.actions.refresh', { ns: 'settings' })}
          >
            <ArrowsClockwiseIcon
              className={
                refreshing ? 'size-icon-sm animate-spin' : 'size-icon-sm'
              }
            />
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={busy || editor !== null}
            onClick={() => setEditor({ mode: 'add' })}
          >
            <PlusIcon className="size-icon-sm" />
            {t('settings.aws.actions.add', { ns: 'settings' })}
          </Button>
        </div>
      </div>

      {loadError && <p className="text-sm text-error">{loadError}</p>}
      {cliMissing && (
        <p className="text-sm text-warning">
          {t('settings.aws.cliMissing', { ns: 'settings' })}
        </p>
      )}
      {profiles !== null && profiles.length === 0 && (
        <p className="text-sm text-low">
          {t('settings.aws.empty', { ns: 'settings' })}
        </p>
      )}

      {editor?.mode === 'add' && (
        <AwsProfileForm
          initial={EMPTY_FORM}
          isNew
          existingNames={profiles?.map((p) => p.profile.name) ?? []}
          onClose={() => setEditor(null)}
          onSaved={() => {
            setEditor(null);
            void refresh();
          }}
        />
      )}

      {importOpen && (
        <AwsProfileImport
          profiles={profiles ?? []}
          onClose={() => setImportOpen(false)}
          onImported={() => {
            setImportOpen(false);
            void refresh();
          }}
        />
      )}

      {profiles?.map((status) => (
        <AwsProfileRow
          key={status.profile.name}
          status={status}
          busy={busy}
          editing={
            editor?.mode === 'edit' &&
            editor.profile.name === status.profile.name
          }
          loginOpen={loginProfile === status.profile.name}
          onEdit={() => setEditor({ mode: 'edit', profile: status.profile })}
          onCloseEdit={() => setEditor(null)}
          onSaved={() => {
            setEditor(null);
            void refresh();
          }}
          onDelete={() => void handleDelete(status.profile.name)}
          onLogin={() =>
            setLoginProfile((current) =>
              current === status.profile.name ? null : status.profile.name
            )
          }
          onStatus={applyStatus}
        />
      ))}
    </SettingsCard>
  );
}

function authBadge(
  status: AwsSsoProfileStatus,
  t: ReturnType<typeof useTranslation>['t']
) {
  switch (status.auth.status) {
    case 'authenticated':
      return (
        <span className="flex items-center gap-1 text-success">
          <CheckCircleIcon className="size-icon-sm shrink-0" weight="fill" />
          {t('settings.aws.auth.authenticated', { ns: 'settings' })}
        </span>
      );
    case 'unauthenticated':
      return (
        <span className="flex items-center gap-1 text-warning">
          <WarningCircleIcon className="size-icon-sm shrink-0" weight="fill" />
          {t('settings.aws.auth.unauthenticated', { ns: 'settings' })}
        </span>
      );
    case 'cli_missing':
      return (
        <span className="text-low">
          {t('settings.aws.auth.cliMissing', { ns: 'settings' })}
        </span>
      );
    default:
      return (
        <span className="text-low">
          {t('settings.aws.auth.unknown', { ns: 'settings' })}
        </span>
      );
  }
}

function AwsProfileImport({
  profiles,
  onClose,
  onImported,
}: {
  profiles: AwsSsoProfileStatus[];
  onClose: () => void;
  onImported: () => void;
}) {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const [sessions, setSessions] = useState<AwsSsoSession[]>([]);
  const [session, setSession] = useState<AwsSsoSession>({
    name: '',
    sso_start_url: '',
    sso_region: '',
  });
  const [step, setStep] = useState<'session' | 'login' | 'catalog'>('session');
  const [rows, setRows] = useState<AwsImportRow[]>([]);
  const [region, setRegion] = useState('us-east-1');
  const [output, setOutput] = useState('json');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<AwsProfileImportResult | null>(null);

  useEffect(() => {
    void machineClient
      ?.listAwsSsoSessions()
      .then(setSessions)
      .catch(() => {});
  }, [machineClient]);

  const updateRow = (key: string, patch: Partial<AwsImportRow>) => {
    setRows((current) =>
      current.map((row) => {
        if (row.key !== key) return row;
        const next = { ...row, ...patch };
        if (patch.name !== undefined) {
          const match = profiles.find((p) => p.profile.name === patch.name);
          next.conflict = !match
            ? 'none'
            : match.editable
              ? 'editable'
              : 'protected';
          next.overwrite = false;
        }
        return next;
      })
    );
  };

  const prepare = async () => {
    if (!machineClient) return;
    const prepared = {
      name: session.name.trim() || defaultAwsSessionName(session.sso_start_url),
      sso_start_url: session.sso_start_url.trim(),
      sso_region: session.sso_region.trim(),
    };
    setBusy(true);
    setError(null);
    try {
      await machineClient.prepareAwsSsoSession(prepared);
      setSession(prepared);
      setStep('login');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const discover = async () => {
    if (!machineClient) return;
    setBusy(true);
    setError(null);
    try {
      const catalog = await machineClient.discoverAwsSsoCatalog(session.name);
      setRows(buildAwsImportRows(catalog, profiles));
      setStep('catalog');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const importProfiles = async () => {
    if (!machineClient) return;
    setBusy(true);
    setError(null);
    try {
      const imported = await machineClient.importAwsProfiles(
        buildAwsImportRequest(session.name, region, output, rows)
      );
      setResult(imported);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const selected = rows.filter((row) => row.selected);
  const blocked = isAwsImportBlocked(rows);

  if (result) {
    return (
      <div className="rounded-sm border border-border p-3 space-y-3">
        <p className="text-sm text-success">
          {t('settings.aws.import.success', {
            ns: 'settings',
            created: result.created.length,
            updated: result.updated.length,
          })}
        </p>
        <div className="flex justify-end">
          <PrimaryButton onClick={onImported}>
            {t('settings.aws.import.done', { ns: 'settings' })}
          </PrimaryButton>
        </div>
      </div>
    );
  }

  return (
    <div className="rounded-sm border border-border p-3 space-y-3">
      <h3 className="text-sm font-medium text-high">
        {t('settings.aws.import.title', { ns: 'settings' })}
      </h3>
      {step === 'session' && (
        <>
          {sessions.length > 0 && (
            <SettingsField
              label={t('settings.aws.import.existingSession', {
                ns: 'settings',
              })}
            >
              <select
                className="w-full rounded-sm border border-border bg-primary px-2 py-1.5 text-sm"
                value={
                  sessions.some((s) => s.name === session.name)
                    ? session.name
                    : ''
                }
                onChange={(event) => {
                  const selected = sessions.find(
                    (item) => item.name === event.target.value
                  );
                  if (selected) setSession(selected);
                }}
              >
                <option value="">
                  {t('settings.aws.import.newSession', { ns: 'settings' })}
                </option>
                {sessions.map((item) => (
                  <option key={item.name} value={item.name}>
                    {item.name}
                  </option>
                ))}
              </select>
            </SettingsField>
          )}
          <SettingsField
            label={t('settings.aws.import.sessionName', { ns: 'settings' })}
          >
            <Input
              value={session.name}
              placeholder="my-org"
              onChange={(event) =>
                setSession((current) => ({
                  ...current,
                  name: event.target.value,
                }))
              }
            />
          </SettingsField>
          <SettingsField
            label={t('settings.aws.form.startUrl', { ns: 'settings' })}
          >
            <Input
              value={session.sso_start_url}
              placeholder="https://my-org.awsapps.com/start"
              onChange={(event) =>
                setSession((current) => ({
                  ...current,
                  sso_start_url: event.target.value,
                }))
              }
            />
          </SettingsField>
          <SettingsField
            label={t('settings.aws.form.ssoRegion', { ns: 'settings' })}
          >
            <Input
              value={session.sso_region}
              placeholder="us-east-1"
              onChange={(event) =>
                setSession((current) => ({
                  ...current,
                  sso_region: event.target.value,
                }))
              }
            />
          </SettingsField>
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={onClose}>
              {t('settings.aws.form.cancel', { ns: 'settings' })}
            </Button>
            <PrimaryButton
              disabled={busy}
              actionIcon={busy ? 'spinner' : undefined}
              onClick={() => void prepare()}
            >
              {t('settings.aws.import.prepare', { ns: 'settings' })}
            </PrimaryButton>
          </div>
        </>
      )}
      {step === 'login' && (
        <>
          <AwsLoginTerminal
            name={session.name}
            sessionLogin
            onStatus={() => {}}
            onComplete={() => void discover()}
          />
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={onClose}>
              {t('settings.aws.form.cancel', { ns: 'settings' })}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => void discover()}
            >
              {t('settings.aws.import.discover', { ns: 'settings' })}
            </Button>
          </div>
        </>
      )}
      {step === 'catalog' && (
        <>
          <div className="flex items-center justify-between gap-2">
            <span className="text-sm text-low">
              {t('settings.aws.import.found', {
                ns: 'settings',
                count: rows.length,
              })}
            </span>
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="ghost"
                onClick={() =>
                  setRows((current) =>
                    current.map((row) => ({ ...row, selected: true }))
                  )
                }
              >
                {t('settings.aws.import.selectAll', { ns: 'settings' })}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() =>
                  setRows((current) =>
                    current.map((row) => ({ ...row, selected: false }))
                  )
                }
              >
                {t('settings.aws.import.clearAll', { ns: 'settings' })}
              </Button>
            </div>
          </div>
          <div className="max-h-80 space-y-2 overflow-y-auto">
            {rows.map((row) => (
              <div
                key={row.key}
                className="rounded-sm border border-border p-2"
              >
                <label className="flex items-start gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={row.selected}
                    onChange={(event) =>
                      updateRow(row.key, { selected: event.target.checked })
                    }
                  />
                  <span>
                    {row.account_name} ({row.sso_account_id}) ·{' '}
                    {row.sso_role_name}
                  </span>
                </label>
                {row.selected && (
                  <div className="mt-2 space-y-1 pl-6">
                    <Input
                      value={row.name}
                      aria-label={t('settings.aws.import.profileName', {
                        ns: 'settings',
                        account: row.account_name,
                        role: row.sso_role_name,
                      })}
                      onChange={(event) =>
                        updateRow(row.key, { name: event.target.value })
                      }
                    />
                    {row.conflict === 'editable' && (
                      <label className="flex items-center gap-2 text-xs text-warning">
                        <input
                          type="checkbox"
                          checked={row.overwrite}
                          onChange={(event) =>
                            updateRow(row.key, {
                              overwrite: event.target.checked,
                            })
                          }
                        />
                        {t('settings.aws.import.confirmOverwrite', {
                          ns: 'settings',
                          name: row.name,
                        })}
                      </label>
                    )}
                    {row.conflict === 'protected' && (
                      <p className="text-xs text-error">
                        {t('settings.aws.import.protectedConflict', {
                          ns: 'settings',
                          name: row.name,
                        })}
                      </p>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <SettingsField
              label={t('settings.aws.form.region', { ns: 'settings' })}
            >
              <Input
                value={region}
                onChange={(event) => setRegion(event.target.value)}
              />
            </SettingsField>
            <SettingsField
              label={t('settings.aws.form.output', { ns: 'settings' })}
            >
              <Input
                value={output}
                onChange={(event) => setOutput(event.target.value)}
              />
            </SettingsField>
          </div>
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={onClose}>
              {t('settings.aws.form.cancel', { ns: 'settings' })}
            </Button>
            <PrimaryButton
              disabled={busy || selected.length === 0 || blocked}
              actionIcon={busy ? 'spinner' : undefined}
              onClick={() => void importProfiles()}
            >
              {t('settings.aws.import.importSelected', {
                ns: 'settings',
                count: selected.length,
              })}
            </PrimaryButton>
          </div>
        </>
      )}
      {error && (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      )}
    </div>
  );
}

function AwsProfileRow({
  status,
  busy,
  editing,
  loginOpen,
  onEdit,
  onCloseEdit,
  onSaved,
  onDelete,
  onLogin,
  onStatus,
}: {
  status: AwsSsoProfileStatus;
  busy: boolean;
  editing: boolean;
  loginOpen: boolean;
  onEdit: () => void;
  onCloseEdit: () => void;
  onSaved: () => void;
  onDelete: () => void;
  onLogin: () => void;
  onStatus: (status: AwsSsoProfileStatus) => void;
}) {
  const { t } = useTranslation(['settings']);
  const loginAction = getAwsProfileLoginAction(status);
  const editable = canEditAwsProfile(status);

  return (
    <div className="rounded-sm border border-border p-3 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-high">
              {status.profile.name}
            </span>
            {!editable && (
              <span className="text-xs text-low">
                {t('settings.aws.readOnly', { ns: 'settings' })}
              </span>
            )}
          </div>
          <p className="text-sm text-low mt-1">
            {status.profile.sso_account_id} · {status.profile.sso_role_name}
            {status.profile.region ? ` · ${status.profile.region}` : ''}
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {loginAction && (
            <Button size="sm" variant="secondary" onClick={onLogin}>
              {loginAction === 'reauthenticate'
                ? t('settings.aws.actions.reauthenticate', { ns: 'settings' })
                : t('settings.aws.actions.signIn', { ns: 'settings' })}
            </Button>
          )}
          {editable && (
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={onEdit}
              aria-label={t('settings.aws.actions.edit', { ns: 'settings' })}
            >
              <PencilSimpleIcon className="size-icon-sm" />
            </Button>
          )}
          {editable && (
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={onDelete}
              aria-label={t('settings.aws.actions.delete', { ns: 'settings' })}
            >
              <TrashIcon className="size-icon-sm" />
            </Button>
          )}
        </div>
      </div>
      <div className="text-sm">
        {authBadge(status, t)}
        {status.auth.status === 'authenticated' && (
          <p className="text-xs text-low mt-1 break-all">
            {status.auth.identity}
          </p>
        )}
        {status.auth.status === 'unknown' && (
          <p className="text-xs text-low mt-1 break-all">
            {status.auth.message}
          </p>
        )}
      </div>
      {editing && (
        <AwsProfileForm
          initial={status.profile}
          isNew={false}
          existingNames={[]}
          onClose={onCloseEdit}
          onSaved={onSaved}
        />
      )}
      {loginOpen && (
        <AwsLoginTerminal name={status.profile.name} onStatus={onStatus} />
      )}
    </div>
  );
}

function validateForm(
  form: AwsSsoProfile,
  isNew: boolean,
  existingNames: string[],
  t: ReturnType<typeof useTranslation>['t']
): string | null {
  if (!NAME_PATTERN.test(form.name) || form.name === 'default') {
    return t('settings.aws.form.errors.name', { ns: 'settings' });
  }
  if (isNew && existingNames.includes(form.name)) {
    return t('settings.aws.form.errors.duplicate', { ns: 'settings' });
  }
  if (!form.sso_start_url.startsWith('https://')) {
    return t('settings.aws.form.errors.startUrl', { ns: 'settings' });
  }
  if (!REGION_PATTERN.test(form.sso_region)) {
    return t('settings.aws.form.errors.ssoRegion', { ns: 'settings' });
  }
  if (!ACCOUNT_PATTERN.test(form.sso_account_id)) {
    return t('settings.aws.form.errors.accountId', { ns: 'settings' });
  }
  if (!ROLE_PATTERN.test(form.sso_role_name)) {
    return t('settings.aws.form.errors.roleName', { ns: 'settings' });
  }
  if (form.region && !REGION_PATTERN.test(form.region)) {
    return t('settings.aws.form.errors.region', { ns: 'settings' });
  }
  if (form.output && !OUTPUTS.includes(form.output)) {
    return t('settings.aws.form.errors.output', { ns: 'settings' });
  }
  return null;
}

/** Dialog-local snapshot: mutations stay in this form until an explicit save. */
function AwsProfileForm({
  initial,
  isNew,
  existingNames,
  onClose,
  onSaved,
}: {
  initial: AwsSsoProfile;
  isNew: boolean;
  existingNames: string[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const [form, setForm] = useState<AwsSsoProfile>(initial);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const set = (patch: Partial<AwsSsoProfile>) =>
    setForm((current) => ({ ...current, ...patch }));

  const handleSave = async () => {
    if (!machineClient) return;
    const trimmed: AwsSsoProfile = {
      ...form,
      name: form.name.trim(),
      sso_start_url: form.sso_start_url.trim(),
      sso_region: form.sso_region.trim(),
      sso_account_id: form.sso_account_id.trim(),
      sso_role_name: form.sso_role_name.trim(),
      region: form.region?.trim() ? form.region.trim() : null,
      output: form.output?.trim() ? form.output.trim() : null,
    };
    const validationError = validateForm(trimmed, isNew, existingNames, t);
    if (validationError) {
      setError(validationError);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await machineClient.saveAwsProfile(trimmed);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="rounded-sm border border-border p-3 space-y-3">
      <SettingsField label={t('settings.aws.form.name', { ns: 'settings' })}>
        <Input
          value={form.name}
          disabled={!isNew}
          placeholder="ai-foundry.AdministratorAccess"
          onChange={(e) => set({ name: e.target.value })}
        />
      </SettingsField>
      <SettingsField
        label={t('settings.aws.form.startUrl', { ns: 'settings' })}
      >
        <Input
          value={form.sso_start_url}
          placeholder="https://my-org.awsapps.com/start"
          onChange={(e) => set({ sso_start_url: e.target.value })}
        />
      </SettingsField>
      <div className="grid grid-cols-2 gap-3">
        <SettingsField
          label={t('settings.aws.form.ssoRegion', { ns: 'settings' })}
        >
          <Input
            value={form.sso_region}
            placeholder="us-east-1"
            onChange={(e) => set({ sso_region: e.target.value })}
          />
        </SettingsField>
        <SettingsField
          label={t('settings.aws.form.accountId', { ns: 'settings' })}
        >
          <Input
            value={form.sso_account_id}
            placeholder="123456789012"
            onChange={(e) => set({ sso_account_id: e.target.value })}
          />
        </SettingsField>
        <SettingsField
          label={t('settings.aws.form.roleName', { ns: 'settings' })}
        >
          <Input
            value={form.sso_role_name}
            placeholder="AdministratorAccess"
            onChange={(e) => set({ sso_role_name: e.target.value })}
          />
        </SettingsField>
        <SettingsField
          label={t('settings.aws.form.region', { ns: 'settings' })}
        >
          <Input
            value={form.region ?? ''}
            placeholder="us-east-1"
            onChange={(e) => set({ region: e.target.value || null })}
          />
        </SettingsField>
        <SettingsField
          label={t('settings.aws.form.output', { ns: 'settings' })}
        >
          <Input
            value={form.output ?? ''}
            placeholder="json"
            onChange={(e) => set({ output: e.target.value || null })}
          />
        </SettingsField>
      </div>
      {error && <p className="text-sm text-error">{error}</p>}
      <div className="flex items-center justify-end gap-2">
        <Button size="sm" variant="ghost" disabled={saving} onClick={onClose}>
          {t('settings.aws.form.cancel', { ns: 'settings' })}
        </Button>
        <PrimaryButton
          disabled={saving}
          actionIcon={saving ? 'spinner' : undefined}
          onClick={() => void handleSave()}
        >
          {t('settings.aws.form.save', { ns: 'settings' })}
        </PrimaryButton>
      </div>
    </div>
  );
}

function AwsLoginTerminal({
  name,
  onStatus,
  sessionLogin = false,
  onComplete,
}: {
  name: string;
  onStatus: (status: AwsSsoProfileStatus) => void;
  sessionLogin?: boolean;
  onComplete?: () => void;
}) {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const containerRef = useRef<HTMLDivElement>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const onStatusRef = useRef(onStatus);
  const onCompleteRef = useRef(onComplete);
  const [result, setResult] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    onStatusRef.current = onStatus;
    onCompleteRef.current = onComplete;
  }, [onComplete, onStatus]);

  useEffect(() => {
    if (!machineClient || !containerRef.current) return;
    setResult(null);
    const terminal = new Terminal({
      cursorBlink: true,
      fontSize: 12,
      fontFamily: '"IBM Plex Mono", monospace',
      theme: getTerminalTheme(),
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.loadAddon(new WebLinksAddon());
    terminal.open(containerRef.current);
    fit.fit();
    let disposed = false;
    let resizeObserver: ResizeObserver | null = new ResizeObserver(() => {
      fit.fit();
      if (socketRef.current?.readyState === WebSocket.OPEN) {
        socketRef.current.send(
          JSON.stringify({
            type: 'resize',
            cols: terminal.cols,
            rows: terminal.rows,
          })
        );
      }
    });
    resizeObserver.observe(containerRef.current);

    const openLogin = sessionLogin
      ? machineClient.openAwsSsoSessionLogin(name)
      : machineClient.openAwsProfileLogin(name);
    void openLogin
      .then((socket) => {
        if (disposed) {
          socket.close();
          return;
        }
        socketRef.current = socket;
        let receivedResult = false;
        socket.onmessage = (event) => {
          const message = JSON.parse(event.data);
          if (message.type === 'output') {
            const bytes = Uint8Array.from(atob(message.data), (char) =>
              char.charCodeAt(0)
            );
            terminal.write(bytes);
          } else if (message.type === 'exit') {
            receivedResult = true;
            setResult(message.outcome);
            if (message.outcome === 'succeeded') onCompleteRef.current?.();
          } else if (message.type === 'status') {
            onStatusRef.current(message.profile as AwsSsoProfileStatus);
          } else if (message.type === 'error') {
            receivedResult = true;
            setResult(message.message);
          }
        };
        // A WebSocket constructor can succeed before its HTTP upgrade is
        // rejected; without these handlers the terminal sticks in "running".
        socket.onerror = () => {
          if (!receivedResult && !disposed) {
            receivedResult = true;
            setResult(
              t('settings.aws.login.connectionFailed', { ns: 'settings' })
            );
          }
        };
        socket.onclose = () => {
          if (!receivedResult && !disposed) {
            receivedResult = true;
            setResult(
              t('settings.aws.login.connectionClosed', { ns: 'settings' })
            );
          }
        };
        terminal.onData((data) => {
          if (socket.readyState === WebSocket.OPEN) {
            const bytes = new TextEncoder().encode(data);
            const binary = Array.from(bytes, (byte) =>
              String.fromCharCode(byte)
            ).join('');
            socket.send(JSON.stringify({ type: 'input', data: btoa(binary) }));
          }
        });
      })
      .catch((error) =>
        setResult(error instanceof Error ? error.message : String(error))
      );

    return () => {
      disposed = true;
      resizeObserver?.disconnect();
      resizeObserver = null;
      socketRef.current?.close();
      socketRef.current = null;
      terminal.dispose();
    };
  }, [attempt, machineClient, name, sessionLogin, t]);

  return (
    <div className="space-y-2 border-t border-border pt-3">
      <div ref={containerRef} className="h-64 w-full rounded-sm bg-black p-1" />
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-low">
          {result ?? t('settings.aws.login.running', { ns: 'settings' })}
        </span>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => {
            if (result !== null) {
              setAttempt((current) => current + 1);
            } else {
              socketRef.current?.send(JSON.stringify({ type: 'cancel' }));
            }
          }}
        >
          {result !== null
            ? t('settings.aws.login.retry', { ns: 'settings' })
            : t('settings.aws.login.cancel', { ns: 'settings' })}
        </Button>
      </div>
    </div>
  );
}
