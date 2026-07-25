/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  PipelineFileStatus,
  PipelineValidation,
  PipelineValidateBody,
} from 'shared/types';
import type { MachineClient } from '@/shared/lib/machineClient';
import { PipelinesSettingsSection } from './PipelinesSettingsSection';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  currentHost: 'host-a',
  rawByHost: {
    'host-a': { basic: 'name = "Basic A"\n' },
    'host-b': { custom: 'name = "Custom B"\n' },
  } as Record<string, Record<string, string>>,
  statusesByHost: {
    'host-a': [
      {
        id: 'basic',
        name: 'Basic',
        valid: true,
        stage_count: 2,
        error: null,
      },
      {
        id: 'broken',
        name: 'broken.toml',
        valid: false,
        stage_count: null,
        error: { message: 'expected table', line: 2, column: 4 },
      },
    ],
    'host-b': [
      {
        id: 'custom',
        name: 'Custom',
        valid: true,
        stage_count: 1,
        error: null,
      },
    ],
  } as Record<string, PipelineFileStatus[]>,
  validate:
    vi.fn<(body: PipelineValidateBody) => Promise<PipelineValidation>>(),
  write: vi.fn(),
  remove: vi.fn(),
  reset: vi.fn(),
  resetAll: vi.fn(),
  confirm: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => {
      if (key === 'settings.pipelines.status.stageCount') {
        return `${values?.count} stages`;
      }
      if (key === 'settings.pipelines.status.location') {
        return `at ${values?.location}`;
      }
      return key;
    },
  }),
}));

vi.mock('@vibe/ui/components/Button', () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock('@vibe/ui/components/ConfirmDialog', () => ({
  ConfirmDialog: {
    show: mocks.confirm,
  },
}));

vi.mock('./SettingsComponents', () => ({
  SettingsCard: ({
    title,
    description,
    headerAction,
    children,
  }: {
    title: string;
    description?: string;
    headerAction?: React.ReactNode;
    children: React.ReactNode;
  }) => (
    <section>
      <h2>{title}</h2>
      {description ? <p>{description}</p> : null}
      {headerAction}
      {children}
    </section>
  ),
  SettingsInput: ({
    value,
    onChange,
    placeholder,
    disabled,
  }: {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    disabled?: boolean;
  }) => (
    <input
      value={value}
      onChange={(event) => onChange(event.currentTarget.value)}
      placeholder={placeholder}
      disabled={disabled}
    />
  ),
}));

vi.mock('./SettingsDirtyContext', () => ({
  useSettingsDirty: () => ({
    setDirty: vi.fn(),
  }),
}));

vi.mock('./SettingsHostContext', () => {
  const clients = new Map<string, MachineClient>();
  return {
    useSettingsMachineClient: (): MachineClient => {
      const existing = clients.get(mocks.currentHost);
      if (existing) {
        return existing;
      }
      const client = {
        target: {
          kind: 'remote',
          id: mocks.currentHost,
          apiHostId: mocks.currentHost,
          label: mocks.currentHost,
        },
        queryScopeKey: ['machine', mocks.currentHost],
      } as MachineClient;
      clients.set(mocks.currentHost, client);
      return client;
    },
  };
});

vi.mock('@/shared/hooks/usePipelines', () => ({
  usePipelineStatuses: (machineClient: MachineClient) => ({
    data: mocks.statusesByHost[machineClient.target.id] ?? [],
    isLoading: false,
    isError: false,
    isSuccess: true,
    error: null,
    refetch: vi.fn(),
  }),
  usePipelineRaw: (
    machineClient: MachineClient,
    pipelineId: string | null
  ) => ({
    data:
      pipelineId == null
        ? undefined
        : mocks.rawByHost[machineClient.target.id]?.[pipelineId],
    isLoading: false,
    isError: false,
    error: null,
    refetch: vi.fn(),
  }),
  useValidatePipelineMutation: () => ({
    mutateAsync: mocks.validate,
  }),
  useWritePipelineRawMutation: () => ({
    mutateAsync: mocks.write,
    isPending: false,
  }),
  useDeletePipelineMutation: () => ({
    mutateAsync: mocks.remove,
    isPending: false,
  }),
  useResetPipelineMutation: () => ({
    mutateAsync: mocks.reset,
    isPending: false,
  }),
  useResetDefaultPipelinesMutation: () => ({
    mutateAsync: mocks.resetAll,
    isPending: false,
  }),
}));

function renderSection() {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);

  act(() => {
    root.render(<PipelinesSettingsSection />);
  });

  return { container, root };
}

function setNativeValue(
  element: HTMLInputElement | HTMLTextAreaElement,
  value: string
) {
  const prototype =
    element instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(prototype, 'value')?.set;
  setter?.call(element, value);
}

function changeTextarea(container: HTMLElement, value: string) {
  const textarea = container.querySelector('textarea');
  if (!textarea) {
    throw new Error('textarea not found');
  }
  act(() => {
    setNativeValue(textarea, value);
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

function changeInput(container: HTMLElement, value: string) {
  const input = container.querySelector('input');
  if (!input) {
    throw new Error('input not found');
  }
  act(() => {
    setNativeValue(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

function clickButton(container: HTMLElement, text: string) {
  const button = Array.from(container.querySelectorAll('button')).find(
    (item) =>
      item.textContent?.includes(text) ||
      item.getAttribute('aria-label') === text
  );
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  act(() => {
    button.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  });
}

describe('PipelinesSettingsSection', () => {
  let root: Root | null = null;
  let container: HTMLElement | null = null;

  beforeEach(() => {
    vi.useFakeTimers();
    mocks.currentHost = 'host-a';
    mocks.validate.mockResolvedValue({ valid: true, error: null });
    mocks.write.mockResolvedValue(undefined);
    mocks.remove.mockResolvedValue(undefined);
    mocks.reset.mockResolvedValue(undefined);
    mocks.resetAll.mockResolvedValue(undefined);
    mocks.confirm.mockResolvedValue('confirmed');
    document.body.innerHTML = '';
  });

  afterEach(() => {
    if (root) {
      act(() => root?.unmount());
    }
    root = null;
    container = null;
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('loads statuses and raw content for the selected host', () => {
    ({ container, root } = renderSection());

    expect(container.textContent).toContain('basic');
    expect(container.textContent).toContain('2 stages');
    expect(container.textContent).toContain('broken');
    expect(container.textContent).toContain('expected table');
    expect(container.textContent).toContain('at 2:4');
    expect(container.querySelector('textarea')?.value).toBe(
      'name = "Basic A"\n'
    );

    mocks.currentHost = 'host-b';
    act(() => {
      root?.render(<PipelinesSettingsSection />);
    });

    expect(container.textContent).toContain('custom');
    expect(container.textContent).not.toContain('broken');
    expect(container.querySelector('textarea')?.value).toBe(
      'name = "Custom B"\n'
    );
  });

  it('ignores stale validation responses for older draft content', async () => {
    ({ container, root } = renderSection());
    const pending: Array<(result: PipelineValidation) => void> = [];
    mocks.validate.mockImplementation(
      () =>
        new Promise<PipelineValidation>((resolve) => {
          pending.push(resolve);
        })
    );

    changeTextarea(container, 'invalid draft');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
    });
    changeTextarea(container, 'valid draft');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
    });

    await act(async () => {
      pending[0]?.({
        valid: false,
        error: { message: 'old error', line: null, column: null },
      });
      await Promise.resolve();
    });
    expect(container.textContent).not.toContain('old error');

    await act(async () => {
      pending[1]?.({ valid: true, error: null });
      await Promise.resolve();
    });
    expect(container.textContent).toContain(
      'settings.pipelines.validation.valid'
    );
  });

  it('rejects add conflicts before opening a draft', () => {
    ({ container, root } = renderSection());

    changeInput(container, 'basic');
    clickButton(container, 'settings.pipelines.actions.add');

    expect(container.textContent).toContain('settings.pipelines.add.conflict');
    expect(container.querySelector('textarea')?.value).toBe(
      'name = "Basic A"\n'
    );
  });

  it('keeps Save disabled for invalid drafts and enables it for valid changes', async () => {
    ({ container, root } = renderSection());
    mocks.validate.mockResolvedValueOnce({
      valid: false,
      error: { message: 'bad toml', line: null, column: null },
    });

    changeTextarea(container, 'bad');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('bad toml');
    expect(
      Array.from(container.querySelectorAll('button'))
        .find((button) =>
          button.textContent?.includes('settings.pipelines.actions.save')
        )
        ?.hasAttribute('disabled')
    ).toBe(true);

    mocks.validate.mockResolvedValueOnce({ valid: true, error: null });
    changeTextarea(container, 'name = "Changed"\n');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
      await Promise.resolve();
    });

    expect(
      Array.from(container.querySelectorAll('button'))
        .find((button) =>
          button.textContent?.includes('settings.pipelines.actions.save')
        )
        ?.hasAttribute('disabled')
    ).toBe(false);
  });

  it('shows save failures without discarding the draft', async () => {
    ({ container, root } = renderSection());
    mocks.validate.mockResolvedValue({ valid: true, error: null });
    mocks.write.mockRejectedValueOnce(new Error('write failed'));

    changeTextarea(container, 'name = "Changed"\n');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
      await Promise.resolve();
    });
    clickButton(container, 'settings.pipelines.actions.save');
    await act(async () => {
      await Promise.resolve();
    });

    expect(container.textContent).toContain('write failed');
    expect(container.querySelector('textarea')?.value).toBe(
      'name = "Changed"\n'
    );
  });

  it('confirms delete and reset actions', async () => {
    ({ container, root } = renderSection());

    clickButton(container, 'settings.pipelines.actions.resetOne');
    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.reset).toHaveBeenCalledWith('basic');

    clickButton(container, 'settings.pipelines.actions.resetAll');
    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.resetAll).toHaveBeenCalled();

    clickButton(container, 'settings.pipelines.actions.delete');
    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.remove).toHaveBeenCalledWith('basic');
  });
});
