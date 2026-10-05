/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BaseCodingAgent } from 'shared/types';
import type { DisableAgentBlocker } from '@/shared/lib/disabledAgents';
import { AgentVisibilityCard } from './AgentVisibilityCard';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;
const onChange = vi.fn();

function render(agentDisabled: boolean, blocker: DisableAgentBlocker | null) {
  act(() =>
    root.render(
      <AgentVisibilityCard
        executor={'QWEN_CODE' as BaseCodingAgent}
        agentDisabled={agentDisabled}
        blocker={blocker}
        onChange={onChange}
      />
    )
  );
  return container.querySelector<HTMLInputElement>('#agent-visible-QWEN_CODE')!;
}

beforeEach(() => {
  onChange.mockReset();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('AgentVisibilityCard', () => {
  it('disables a shown agent when unchecked', () => {
    const box = render(false, null);
    expect(box.checked).toBe(true);
    act(() => box.click());
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('re-enables a disabled agent when checked', () => {
    const box = render(true, null);
    expect(box.checked).toBe(false);
    act(() => box.click());
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it.each([
    ['default', 'settings.agents.visibility.defaultLocked'],
    ['last', 'settings.agents.visibility.lastLocked'],
  ] as const)('locks the %s agent and says why', (blocker, reasonKey) => {
    const box = render(false, blocker);
    expect(box.disabled).toBe(true);
    expect(container.textContent).toContain(reasonKey);
  });
});
