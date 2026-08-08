import { describe, expect, it, vi } from 'vitest';
import type { ExecutorConfig } from 'shared/types';
import {
  RESTART_FOR_MCP_PROMPT,
  restartAgentForMcpChanges,
} from './restartAgentForMcpChanges';

const executorConfig = { executor: 'CODEX' } as ExecutorConfig;

function fixture(
  overrides: Partial<Parameters<typeof restartAgentForMcpChanges>[0]> = {}
) {
  return {
    isRunning: false,
    executorConfig,
    confirmQueue: vi.fn().mockResolvedValue(true),
    queueRestart: vi.fn().mockResolvedValue('started' as const),
    ...overrides,
  };
}

describe('restartAgentForMcpChanges', () => {
  it('starts a stopped session immediately', async () => {
    const options = fixture();

    await expect(restartAgentForMcpChanges(options)).resolves.toBe('started');
    expect(options.queueRestart).toHaveBeenCalledOnce();
    expect(options.confirmQueue).not.toHaveBeenCalled();
  });

  it('does not queue when running confirmation is canceled', async () => {
    const options = fixture({
      isRunning: true,
      confirmQueue: vi.fn().mockResolvedValue(false),
    });

    await expect(restartAgentForMcpChanges(options)).resolves.toBe('canceled');
    expect(options.queueRestart).not.toHaveBeenCalled();
  });

  it('queues a fresh-process continuation after a running turn', async () => {
    const options = fixture({
      isRunning: true,
      queueRestart: vi.fn().mockResolvedValue('queued' as const),
    });

    await expect(restartAgentForMcpChanges(options)).resolves.toBe('queued');
    expect(options.queueRestart).toHaveBeenCalledWith(
      RESTART_FOR_MCP_PROMPT,
      executorConfig,
      true
    );
  });

  it('preserves an existing queued user follow-up', async () => {
    const options = fixture({
      isRunning: true,
      queueRestart: vi.fn().mockResolvedValue('queued' as const),
    });

    await expect(restartAgentForMcpChanges(options)).resolves.toBe('queued');
    expect(options.queueRestart).toHaveBeenCalledOnce();
  });

  it('starts immediately if the running turn finishes during confirmation', async () => {
    const options = fixture({
      isRunning: true,
      queueRestart: vi.fn().mockResolvedValue('started' as const),
    });

    await expect(restartAgentForMcpChanges(options)).resolves.toBe('started');
    expect(options.queueRestart).toHaveBeenCalledOnce();
  });

  it('confirms and retries when authoritative state reports a running turn', async () => {
    const options = fixture({
      queueRestart: vi
        .fn()
        .mockResolvedValueOnce('confirmation_required' as const)
        .mockResolvedValueOnce('queued' as const),
    });

    await expect(restartAgentForMcpChanges(options)).resolves.toBe('queued');
    expect(options.confirmQueue).toHaveBeenCalledOnce();
    expect(options.queueRestart).toHaveBeenNthCalledWith(
      2,
      RESTART_FOR_MCP_PROMPT,
      executorConfig,
      true
    );
  });
});
