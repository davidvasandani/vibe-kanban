import { describe, expect, it } from 'vitest';
import {
  buildActivityGroups,
  isInProgressStatus,
  isWorkspaceActive,
  partitionByActivity,
} from './activityGrouping';

describe('isInProgressStatus', () => {
  it('matches the seeded status name', () => {
    expect(isInProgressStatus('In progress')).toBe(true);
  });

  it('is case- and whitespace-tolerant', () => {
    expect(isInProgressStatus('in progress')).toBe(true);
    expect(isInProgressStatus('In Progress')).toBe(true);
    expect(isInProgressStatus('  In progress  ')).toBe(true);
  });

  it('does not match other statuses', () => {
    expect(isInProgressStatus('In review')).toBe(false);
    expect(isInProgressStatus('To do')).toBe(false);
    expect(isInProgressStatus('Doing')).toBe(false);
    expect(isInProgressStatus('inprogress')).toBe(false);
  });
});

describe('isWorkspaceActive', () => {
  it('is active while the agent runs unattended', () => {
    expect(isWorkspaceActive({ isRunning: true })).toBe(true);
    expect(
      isWorkspaceActive({ isRunning: true, hasPendingApproval: false })
    ).toBe(true);
  });

  it('treats a run paused on tool approval as waiting', () => {
    expect(
      isWorkspaceActive({ isRunning: true, hasPendingApproval: true })
    ).toBe(false);
  });

  it('is waiting when not running or when flags are unknown', () => {
    expect(isWorkspaceActive({ isRunning: false })).toBe(false);
    expect(isWorkspaceActive({})).toBe(false);
    expect(isWorkspaceActive({ hasPendingApproval: false })).toBe(false);
  });
});

describe('partitionByActivity', () => {
  it('moves active issues first, preserving order within each group', () => {
    const result = partitionByActivity(
      ['a', 'b', 'c', 'd', 'e'],
      new Set(['b', 'd'])
    );
    expect(result).toEqual(['b', 'd', 'a', 'c', 'e']);
  });

  it('keeps order unchanged when all issues are active', () => {
    expect(
      partitionByActivity(['a', 'b'], new Set(['a', 'b', 'other']))
    ).toEqual(['a', 'b']);
  });

  it('keeps order unchanged when no issues are active', () => {
    expect(partitionByActivity(['a', 'b'], new Set())).toEqual(['a', 'b']);
  });

  it('handles empty columns', () => {
    expect(partitionByActivity([], new Set(['a']))).toEqual([]);
  });
});

describe('buildActivityGroups', () => {
  it('splits ids into groups and shows headers when both are present', () => {
    const groups = buildActivityGroups(
      ['b', 'd', 'a', 'c'],
      new Set(['b', 'd'])
    );
    expect(groups.active).toEqual(['b', 'd']);
    expect(groups.waiting).toEqual(['a', 'c']);
    expect(groups.showHeaders).toBe(true);
  });

  it('hides headers when the column is all active', () => {
    const groups = buildActivityGroups(['a', 'b'], new Set(['a', 'b']));
    expect(groups.showHeaders).toBe(false);
  });

  it('hides headers when the column is all waiting', () => {
    const groups = buildActivityGroups(['a', 'b'], new Set());
    expect(groups.showHeaders).toBe(false);
  });

  it('hides headers for an empty column', () => {
    const groups = buildActivityGroups([], new Set());
    expect(groups.showHeaders).toBe(false);
  });
});
