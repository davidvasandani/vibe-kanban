import { useEffect, useMemo, useState } from 'react';
import { create, useModal } from '@ebay/nice-modal-react';
import { useTranslation } from 'react-i18next';
import { PlusIcon, TrashIcon } from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import { Input } from '@vibe/ui/components/Input';
import { Label } from '@vibe/ui/components/Label';
import { Textarea } from '@vibe/ui/components/Textarea';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import { Alert, AlertDescription } from '@vibe/ui/components/Alert';
import type { JsonValue } from 'shared/types';
import {
  argsFromLines,
  type KeyValue,
  type McpServerCodec,
  type McpServerFormValues,
  type McpTransport,
} from '@/shared/lib/mcpServerCodec';
import { cn } from '@/shared/lib/utils';
import { defineModal } from '@/shared/lib/modals';
import { SettingsSelect } from './SettingsComponents';

export interface McpServerDialogProps {
  codec: McpServerCodec;
  /** Names already used by other servers (for uniqueness validation). */
  existingNames: string[];
  /** Present when editing an existing server. */
  initial?: { name: string; entry: JsonValue };
}

export type McpServerDialogResult =
  | { name: string; entry: JsonValue }
  | undefined;

const TRANSPORT_LABEL: Record<McpTransport, string> = {
  stdio: 'stdio (command)',
  http: 'HTTP',
  sse: 'SSE',
};

function emptyForm(transport: McpTransport): McpServerFormValues {
  return {
    transport,
    command: '',
    args: [],
    env: [],
    url: '',
    headers: [],
  };
}

// Reusable key/value rows editor (env vars, headers).
function KeyValueRows({
  rows,
  onChange,
  keyPlaceholder,
  valuePlaceholder,
  addLabel,
}: {
  rows: KeyValue[];
  onChange: (rows: KeyValue[]) => void;
  keyPlaceholder: string;
  valuePlaceholder: string;
  addLabel: string;
}) {
  const update = (i: number, patch: Partial<KeyValue>) =>
    onChange(rows.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));
  const remove = (i: number) => onChange(rows.filter((_, idx) => idx !== i));
  const add = () => onChange([...rows, { key: '', value: '' }]);

  return (
    <div className="space-y-2">
      {rows.map((row, i) => (
        <div key={i} className="flex items-center gap-2">
          <Input
            value={row.key}
            onChange={(e) => update(i, { key: e.target.value })}
            placeholder={keyPlaceholder}
            className="font-mono w-1/3"
            autoComplete="off"
          />
          <Input
            value={row.value}
            onChange={(e) => update(i, { value: e.target.value })}
            placeholder={valuePlaceholder}
            className="flex-1"
            autoComplete="off"
          />
          <button
            type="button"
            onClick={() => remove(i)}
            aria-label={`Remove ${row.key || keyPlaceholder}`}
            className={cn(
              'flex items-center justify-center p-2 rounded-sm shrink-0',
              'text-error hover:bg-error/10'
            )}
          >
            <TrashIcon className="size-icon-xs" weight="bold" />
          </button>
        </div>
      ))}
      <Button variant="outline" size="sm" onClick={add} type="button">
        <PlusIcon className="size-icon-xs mr-1" weight="bold" />
        {addLabel}
      </Button>
    </div>
  );
}

const McpServerDialogImpl = create<McpServerDialogProps>(
  ({ codec, existingNames, initial }) => {
    const modal = useModal();
    const { t } = useTranslation('settings');

    const isEdit = !!initial;
    // A "custom" entry is one the form can't represent; edit it as raw JSON.
    const initialForm = useMemo(
      () => (initial ? codec.parse(initial.entry) : null),
      [codec, initial]
    );
    const isCustom = isEdit && initialForm === null;

    const [name, setName] = useState(initial?.name ?? '');
    const [form, setForm] = useState<McpServerFormValues>(
      initialForm ?? emptyForm(codec.transports[0])
    );
    const [argsText, setArgsText] = useState(
      (initialForm?.args ?? []).join('\n')
    );
    const [customJson, setCustomJson] = useState(
      isCustom ? JSON.stringify(initial!.entry, null, 2) : ''
    );
    const [error, setError] = useState<string | null>(null);

    // NiceModal keeps this component mounted and reuses it across opens, so
    // useState initializers don't re-run. Re-seed all editable state whenever
    // the dialog becomes visible (or is reopened for a different server).
    useEffect(() => {
      if (!modal.visible) return;
      setName(initial?.name ?? '');
      setForm(initialForm ?? emptyForm(codec.transports[0]));
      setArgsText((initialForm?.args ?? []).join('\n'));
      setCustomJson(isCustom ? JSON.stringify(initial!.entry, null, 2) : '');
      setError(null);
      // initialForm/isCustom derive from codec + initial.
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [modal.visible, codec, initial]);

    const patch = (p: Partial<McpServerFormValues>) =>
      setForm((f) => ({ ...f, ...p }));

    const validate = (): { name: string; entry: JsonValue } | null => {
      const trimmedName = name.trim();
      if (!trimmedName) {
        setError(t('settings.mcp.validation.nameRequired'));
        return null;
      }
      if (trimmedName !== name) {
        setError(t('settings.mcp.validation.nameWhitespace'));
        return null;
      }
      if (
        trimmedName !== initial?.name &&
        existingNames.includes(trimmedName)
      ) {
        setError(t('settings.mcp.validation.nameTaken'));
        return null;
      }

      if (isCustom) {
        try {
          const parsed = JSON.parse(customJson);
          if (
            typeof parsed !== 'object' ||
            parsed === null ||
            Array.isArray(parsed)
          ) {
            setError(t('settings.mcp.validation.customNotObject'));
            return null;
          }
          return { name: trimmedName, entry: parsed as JsonValue };
        } catch {
          setError(t('settings.mcp.validation.invalidJson'));
          return null;
        }
      }

      const args = argsFromLines(argsText);
      if (form.transport === 'stdio') {
        if (!form.command.trim()) {
          setError(t('settings.mcp.validation.commandRequired'));
          return null;
        }
      } else {
        if (!form.url.trim()) {
          setError(t('settings.mcp.validation.urlRequired'));
          return null;
        }
        try {
          const parsed = new URL(form.url);
          if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
            setError(t('settings.mcp.validation.urlScheme'));
            return null;
          }
        } catch {
          setError(t('settings.mcp.validation.urlInvalid'));
          return null;
        }
      }

      const hasDuplicateKey = (rows: KeyValue[]): boolean => {
        const seen = new Set<string>();
        for (const { key } of rows) {
          const k = key.trim();
          if (!k) continue;
          if (seen.has(k)) return true;
          seen.add(k);
        }
        return false;
      };
      if (
        (form.transport === 'stdio' && hasDuplicateKey(form.env)) ||
        (form.transport !== 'stdio' && hasDuplicateKey(form.headers))
      ) {
        setError(t('settings.mcp.validation.duplicateKey'));
        return null;
      }

      const entry = codec.serialize({ ...form, args }, initial?.entry);
      return { name: trimmedName, entry };
    };

    const handleSave = () => {
      const result = validate();
      if (!result) return;
      modal.resolve(result);
      modal.hide();
    };

    const handleCancel = () => {
      modal.resolve(undefined);
      modal.hide();
    };

    const transportOptions = codec.transports.map((tr) => ({
      value: tr,
      label: TRANSPORT_LABEL[tr],
    }));

    return (
      <Dialog
        open={modal.visible}
        onOpenChange={(open) => !open && handleCancel()}
      >
        <DialogContent className="sm:max-w-lg max-h-[85vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>
              {isEdit
                ? t('settings.mcp.dialog.editTitle')
                : t('settings.mcp.dialog.addTitle')}
            </DialogTitle>
            <DialogDescription>
              {t('settings.mcp.dialog.description')}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="mcp-server-name">
                {t('settings.mcp.dialog.name')}
              </Label>
              <Input
                id="mcp-server-name"
                value={name}
                onChange={(e) => {
                  setName(e.target.value);
                  setError(null);
                }}
                placeholder={t('settings.mcp.dialog.namePlaceholder')}
                className="font-mono"
                autoComplete="off"
                autoFocus
              />
            </div>

            {isCustom ? (
              <div className="space-y-2">
                <Label htmlFor="mcp-custom-json">
                  {t('settings.mcp.dialog.customJson')}
                </Label>
                <Textarea
                  id="mcp-custom-json"
                  value={customJson}
                  onChange={(e) => {
                    setCustomJson(e.target.value);
                    setError(null);
                  }}
                  rows={10}
                  className="font-mono border-border rounded-sm"
                />
                <p className="text-xs text-low">
                  {t('settings.mcp.dialog.customJsonHelper')}
                </p>
              </div>
            ) : (
              <>
                {transportOptions.length > 1 && (
                  <div className="space-y-2">
                    <Label>{t('settings.mcp.dialog.transport')}</Label>
                    <SettingsSelect<McpTransport>
                      value={form.transport}
                      options={transportOptions}
                      onChange={(value) => {
                        patch({ transport: value });
                        setError(null);
                      }}
                    />
                  </div>
                )}

                {form.transport === 'stdio' ? (
                  <>
                    <div className="space-y-2">
                      <Label htmlFor="mcp-command">
                        {t('settings.mcp.dialog.command')}
                      </Label>
                      <Input
                        id="mcp-command"
                        value={form.command}
                        onChange={(e) => {
                          patch({ command: e.target.value });
                          setError(null);
                        }}
                        placeholder="npx"
                        className="font-mono"
                        autoComplete="off"
                      />
                    </div>
                    <div className="space-y-2">
                      <Label htmlFor="mcp-args">
                        {t('settings.mcp.dialog.args')}
                      </Label>
                      <Textarea
                        id="mcp-args"
                        value={argsText}
                        onChange={(e) => {
                          setArgsText(e.target.value);
                          setError(null);
                        }}
                        rows={4}
                        placeholder={'-y\nsome-mcp-server@latest'}
                        className="font-mono border-border rounded-sm"
                      />
                      <p className="text-xs text-low">
                        {t('settings.mcp.dialog.argsHelper')}
                      </p>
                    </div>
                    <div className="space-y-2">
                      <Label>{t('settings.mcp.dialog.env')}</Label>
                      <KeyValueRows
                        rows={form.env}
                        onChange={(env) => {
                          patch({ env });
                          setError(null);
                        }}
                        keyPlaceholder="KEY"
                        valuePlaceholder="value"
                        addLabel={t('settings.mcp.dialog.addEnv')}
                      />
                    </div>
                  </>
                ) : (
                  <>
                    <div className="space-y-2">
                      <Label htmlFor="mcp-url">
                        {t('settings.mcp.dialog.url')}
                      </Label>
                      <Input
                        id="mcp-url"
                        value={form.url}
                        onChange={(e) => {
                          patch({ url: e.target.value });
                          setError(null);
                        }}
                        placeholder="https://mcp.example.com/mcp"
                        className="font-mono"
                        autoComplete="off"
                      />
                    </div>
                    <div className="space-y-2">
                      <Label>{t('settings.mcp.dialog.headers')}</Label>
                      <KeyValueRows
                        rows={form.headers}
                        onChange={(headers) => {
                          patch({ headers });
                          setError(null);
                        }}
                        keyPlaceholder="Header-Name"
                        valuePlaceholder="value"
                        addLabel={t('settings.mcp.dialog.addHeader')}
                      />
                    </div>
                  </>
                )}
              </>
            )}

            {error && (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={handleCancel}>
              {t('settings.mcp.dialog.cancel')}
            </Button>
            <Button onClick={handleSave} disabled={!name.trim()}>
              {isEdit
                ? t('settings.mcp.dialog.saveEdit')
                : t('settings.mcp.dialog.add')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const McpServerDialog = defineModal<
  McpServerDialogProps,
  McpServerDialogResult
>(McpServerDialogImpl);
