/* @vitest-environment jsdom */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Pipeline } from 'shared/types';
import { PipelineSection, type PipelineSelection } from './PipelineSection';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const pipelines: Pipeline[] = [
  {
    id: 'basic',
    name: 'Basic',
    description: null,
    stages: [
      {
        id: 'spec',
        label: 'Create spec',
        prompt_fragment: 'Write a spec.',
        default_enabled: true,
        heavy: false,
      },
    ],
  },
  {
    id: 'wikillm',
    name: 'WikiLLM',
    description: null,
    stages: [
      {
        id: 'recall',
        label: 'Recall',
        prompt_fragment: 'Recall prior knowledge.',
        default_enabled: true,
        heavy: false,
      },
    ],
  },
];

vi.mock('@/shared/hooks/usePipelines', () => ({
  usePipelines: () => ({ data: pipelines }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) =>
      key === 'taskPipeline.collapsedSummary'
        ? `· ${String(values?.names)}`
        : key,
  }),
}));

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

function header(): HTMLButtonElement {
  return container.querySelector('button[aria-expanded]') as HTMLButtonElement;
}

function render(
  defaultPipelineIds: readonly string[] | null | undefined,
  onChange: (selection: PipelineSelection) => void = () => {}
) {
  act(() => {
    root.render(
      <PipelineSection
        onChange={onChange}
        defaultPipelineIds={defaultPipelineIds}
      />
    );
  });
}

describe('PipelineSection', () => {
  it('starts collapsed and names the selected pipelines in the header', () => {
    render(['wikillm']);
    expect(header().getAttribute('aria-expanded')).toBe('false');
    expect(header().textContent).toContain('· WikiLLM');
    expect(container.querySelector('textarea')).toBeNull();
  });

  it('emits the seeded block while collapsed', () => {
    const onChange = vi.fn<(selection: PipelineSelection) => void>();
    render(['wikillm'], onChange);
    const last = onChange.mock.calls.at(-1)?.[0];
    expect(last?.pipelineIds).toEqual(['wikillm']);
    expect(last?.enabledIds).toEqual(['recall']);
    expect(last?.block).toContain('Recall prior knowledge.');
  });

  it('waits for the defaults instead of seeding a guess', () => {
    render(null);
    expect(header().textContent).not.toContain('·');
    render(['wikillm']);
    expect(header().textContent).toContain('· WikiLLM');
  });

  it('keeps the controls disabled until the defaults arrive', () => {
    render(null);
    act(() => header().click());
    const boxes = () =>
      Array.from(
        container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')
      );
    expect(boxes().length).toBeGreaterThan(0);
    expect(boxes().every((b) => b.disabled)).toBe(true);
    render(['wikillm']);
    expect(boxes().every((b) => !b.disabled)).toBe(true);
  });

  it('seeds nothing for an empty default list', () => {
    render([]);
    expect(header().textContent).not.toContain('·');
  });

  it('falls back to basic when no defaults are supplied', () => {
    render(undefined);
    expect(header().textContent).toContain('· Basic');
  });

  it('expands on click and hides the summary', () => {
    render(['basic', 'wikillm']);
    act(() => header().click());
    expect(header().getAttribute('aria-expanded')).toBe('true');
    expect(header().textContent).not.toContain('·');
    const boxes = Array.from(
      container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')
    );
    // Two pipeline boxes + two stage boxes, all ticked.
    expect(boxes).toHaveLength(4);
    expect(boxes.every((b) => b.checked)).toBe(true);
  });
});
