/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { EarlierHistoryControl } from './EarlierHistoryControl';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

function renderControl({
  isLoading = false,
  error = null,
  onLoad = vi.fn(),
}: {
  isLoading?: boolean;
  error?: string | null;
  onLoad?: () => void;
} = {}) {
  act(() => {
    root.render(
      <EarlierHistoryControl
        isLoading={isLoading}
        error={error}
        onLoad={onLoad}
      />
    );
  });

  const control = container.querySelector('[data-earlier-history-control]');
  if (!(control instanceof HTMLDivElement)) {
    throw new Error('Expected earlier-history control');
  }
  return control;
}

describe('EarlierHistoryControl', () => {
  it('reserves the same block geometry while idle and loading', () => {
    const idle = renderControl();
    const sizingLayers = idle.querySelectorAll('[data-earlier-history-sizer]');
    expect(sizingLayers).toHaveLength(2);
    sizingLayers.forEach((layer) => {
      expect(layer.classList).toContain('col-start-1');
      expect(layer.classList).toContain('row-start-1');
      expect(layer.classList).toContain('invisible');
      expect(layer.getAttribute('aria-hidden')).toBe('true');
    });
    expect(idle.querySelector('.animate-pulse')).toBeNull();
    expect(idle.querySelector('button')).not.toBeNull();

    const loading = renderControl({ isLoading: true });
    expect(loading).toBe(idle);
    expect(
      loading.querySelectorAll('[data-earlier-history-sizer]')
    ).toHaveLength(2);
    expect(loading.querySelector('button')).toBeNull();
    expect(loading.querySelector('[role="status"]')).not.toBeNull();
    expect(loading.querySelectorAll('.animate-pulse')).toHaveLength(4);
    expect(loading.textContent).toContain(
      'conversation.loadingEarlierMessages'
    );
  });

  it('invokes loading and exposes retry feedback', () => {
    const onLoad = vi.fn();
    const idle = renderControl({ onLoad });
    const button = idle.querySelector('button');
    if (!(button instanceof HTMLButtonElement)) {
      throw new Error('Expected load-earlier button');
    }

    act(() => button.click());
    expect(onLoad).toHaveBeenCalledOnce();

    const retry = renderControl({ error: 'History unavailable', onLoad });
    expect(retry.textContent).toContain('conversation.retryEarlierMessages');
    expect(retry.querySelector('[role="status"]')?.textContent).toContain(
      'conversation.loadEarlierMessagesError'
    );

    const errorRow = retry.querySelector('[data-earlier-history-error]');
    expect(errorRow?.classList).not.toContain('invisible');

    const retryLoading = renderControl({
      isLoading: true,
      error: 'History unavailable',
      onLoad,
    });
    expect(retryLoading.querySelector('[data-earlier-history-error]')).toBe(
      errorRow
    );
    expect(errorRow?.classList).toContain('invisible');
    expect(errorRow?.getAttribute('aria-hidden')).toBe('true');
  });
});
