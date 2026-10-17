import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { ExecutionProcessStatus, type ExecutionProcess } from 'shared/types';
import { PollersSectionContainer } from './PollersSectionContainer';

vi.mock('@/shared/lib/api', () => ({ executionProcessesApi: {} }));

function renderRules(rules: object) {
  const process = {
    id: 'poller',
    run_reason: 'backgroundhelper',
    status: ExecutionProcessStatus.running,
    started_at: '2026-09-15T00:00:00Z',
    executor_action: {
      typ: {
        type: 'ScriptRequest',
        poller: {
          command: 'git fetch',
          interval_secs: 60,
          ...rules,
        },
      },
    },
  } as ExecutionProcess;
  return renderToStaticMarkup(
    <PollersSectionContainer executionProcesses={[process]} />
  );
}

describe('poller stopping details', () => {
  it('shows both configured stopping rules and preserves manual stop', () => {
    const html = renderRules({
      stop_command: 'test -f done',
      timeout_secs: 600,
    });
    expect(html).toContain('time limit 600s');
    expect(html).toContain('stops when successful:');
    expect(html).toContain('test -f done');
    expect(html).toContain('Stop poller: git fetch');
  });
  it('renders legacy pollers without inventing a stopping rule', () => {
    const html = renderRules({});
    expect(html).toContain('git fetch');
    expect(html).not.toContain('time limit');
    expect(html).not.toContain('stops when successful:');
  });
});
