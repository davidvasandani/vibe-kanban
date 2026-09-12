import type { ExecutorConfig } from 'shared/types';

export const RESTART_FOR_MCP_PROMPT =
  'MCP configuration changed. Continue the existing task using the refreshed tool configuration.';

interface RestartAgentForMcpChangesOptions {
  isRunning: boolean;
  executorConfig: ExecutorConfig;
  confirmQueue: () => Promise<boolean>;
  queueRestart: (
    message: string,
    executorConfig: ExecutorConfig,
    confirmedRunningRestart: boolean
  ) => Promise<'confirmation_required' | 'queued' | 'started'>;
}

export type RestartAgentForMcpChangesResult =
  | 'canceled'
  | 'queued'
  | 'started'
  | 'failed';

export async function restartAgentForMcpChanges({
  isRunning,
  executorConfig,
  confirmQueue,
  queueRestart,
}: RestartAgentForMcpChangesOptions): Promise<RestartAgentForMcpChangesResult> {
  let confirmedRunningRestart = false;
  if (isRunning) {
    if (!(await confirmQueue())) return 'canceled';
    confirmedRunningRestart = true;
  }

  let queueResult = await queueRestart(
    RESTART_FOR_MCP_PROMPT,
    executorConfig,
    confirmedRunningRestart
  );
  if (queueResult === 'confirmation_required') {
    if (!(await confirmQueue())) return 'canceled';
    queueResult = await queueRestart(
      RESTART_FOR_MCP_PROMPT,
      executorConfig,
      true
    );
  }
  return queueResult === 'started' ? 'started' : 'queued';
}
