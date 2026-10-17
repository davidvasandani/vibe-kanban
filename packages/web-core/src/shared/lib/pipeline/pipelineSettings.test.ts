import { describe, expect, it } from 'vitest';
import type {
  NewIssuePipelineDefaults,
  PipelineFileStatus,
} from 'shared/types';
import {
  clearScopeOverride,
  createPipelineStarterToml,
  getScopeOverride,
  setScopeOverride,
  toggleMachineDefault,
  togglePipelineId,
  toggleScopePipeline,
  formatPipelineErrorLocation,
  isBundledPipelineId,
  isValidPipelineId,
  selectPipelineAfterRefresh,
  validationTupleMatches,
} from './pipelineSettings';

const status = (id: string): PipelineFileStatus => ({
  id,
  name: id,
  stage_count: 1,
  valid: true,
  error: null,
});

describe('pipeline settings helpers', () => {
  it('accepts ASCII alphanumeric, hyphen, and underscore ids', () => {
    expect(isValidPipelineId('basic')).toBe(true);
    expect(isValidPipelineId('pipeline_2')).toBe(true);
    expect(isValidPipelineId('PIPELINE-2')).toBe(true);
  });

  it('rejects empty, whitespace, path-like, dotted, and non-ASCII ids', () => {
    expect(isValidPipelineId('')).toBe(false);
    expect(isValidPipelineId('my pipeline')).toBe(false);
    expect(isValidPipelineId('../pipeline')).toBe(false);
    expect(isValidPipelineId('pipeline.toml')).toBe(false);
    expect(isValidPipelineId('café')).toBe(false);
  });

  it('detects bundled default ids', () => {
    expect(isBundledPipelineId('basic')).toBe(true);
    expect(isBundledPipelineId('wikillm')).toBe(true);
    expect(isBundledPipelineId('speckit')).toBe(true);
    expect(isBundledPipelineId('parallel-subagents')).toBe(true);
    expect(isBundledPipelineId('custom')).toBe(false);
  });

  it('formats 1-based error locations only when line and column exist', () => {
    expect(formatPipelineErrorLocation({ line: 3, column: 9 })).toBe('3:9');
    expect(formatPipelineErrorLocation({ line: null, column: 9 })).toBeNull();
    expect(formatPipelineErrorLocation({ line: 3, column: null })).toBeNull();
    expect(formatPipelineErrorLocation(null)).toBeNull();
  });

  it('generates the raw starter template without extra rewriting', () => {
    expect(createPipelineStarterToml('new_pipeline'))
      .toBe(`name = "new_pipeline"
description = ""

[[stage]]
id = "stage-1"
label = "Stage 1"
prompt = "Describe what this stage should do."
`);
  });

  it('escapes the id when using it as a starter display name', () => {
    expect(createPipelineStarterToml('quote"slash\\')).toContain(
      'name = "quote\\"slash\\\\"'
    );
  });

  it('compares validation tuples by scope, id, and content', () => {
    const tuple = {
      scopeKey: ['machine', 'a'],
      id: 'basic',
      content: 'name = "Basic"',
    };

    expect(validationTupleMatches(tuple, { ...tuple })).toBe(true);
    expect(
      validationTupleMatches(tuple, { ...tuple, scopeKey: ['machine', 'b'] })
    ).toBe(false);
    expect(validationTupleMatches(tuple, { ...tuple, id: 'other' })).toBe(
      false
    );
    expect(validationTupleMatches(tuple, { ...tuple, content: 'x' })).toBe(
      false
    );
    expect(validationTupleMatches(tuple, null)).toBe(false);
  });

  it('keeps the current id after refresh or falls back to the first status', () => {
    const statuses = [status('basic'), status('custom')];

    expect(selectPipelineAfterRefresh(statuses, 'custom')).toBe('custom');
    expect(selectPipelineAfterRefresh(statuses, 'missing')).toBe('basic');
    expect(selectPipelineAfterRefresh(statuses, null)).toBe('basic');
    expect(selectPipelineAfterRefresh([], 'basic')).toBeNull();
    expect(selectPipelineAfterRefresh(null, 'basic')).toBeNull();
  });
});

describe('new-issue pipeline default updaters', () => {
  const order = ['basic', 'wikillm', 'speckit'];
  const base: NewIssuePipelineDefaults = {
    pipeline_ids: ['basic'],
    organizations: {},
    projects: {},
  };

  it('keeps toggled ids in catalog order, missing ids last', () => {
    expect(togglePipelineId(['speckit'], 'basic', true, order)).toEqual([
      'basic',
      'speckit',
    ]);
    expect(togglePipelineId(['gone', 'basic'], 'wikillm', true, order)).toEqual(
      ['basic', 'wikillm', 'gone']
    );
    expect(togglePipelineId(['gone', 'basic'], 'gone', false, order)).toEqual([
      'basic',
    ]);
  });

  it('toggles the machine default without touching overrides', () => {
    const next = toggleMachineDefault(base, 'basic', false, order);
    expect(next.pipeline_ids).toEqual([]);
    expect(next.projects).toBe(base.projects);
    expect(base.pipeline_ids).toEqual(['basic']);
  });

  it('distinguishes an empty override from inheriting', () => {
    expect(getScopeOverride(base, 'projects', 'p1')).toBeUndefined();
    const none = setScopeOverride(base, 'projects', 'p1', []);
    expect(getScopeOverride(none, 'projects', 'p1')).toEqual([]);
    const inherit = clearScopeOverride(none, 'projects', 'p1');
    expect(getScopeOverride(inherit, 'projects', 'p1')).toBeUndefined();
    expect('p1' in inherit.projects).toBe(false);
  });

  it('toggles pipelines inside an override, creating it if needed', () => {
    const one = toggleScopePipeline(
      base,
      'organizations',
      'o1',
      'speckit',
      true,
      order
    );
    const two = toggleScopePipeline(
      one,
      'organizations',
      'o1',
      'wikillm',
      true,
      order
    );
    expect(two.organizations.o1).toEqual(['wikillm', 'speckit']);
    const emptied = toggleScopePipeline(
      toggleScopePipeline(two, 'organizations', 'o1', 'wikillm', false, order),
      'organizations',
      'o1',
      'speckit',
      false,
      order
    );
    // Unticking everything leaves an explicit "none", not an inherit.
    expect(emptied.organizations.o1).toEqual([]);
  });
});
