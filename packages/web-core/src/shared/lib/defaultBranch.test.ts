import { describe, it, expect } from 'vitest';
import type { GitBranch } from 'shared/types';
import { resolveDefaultBranch } from './defaultBranch';

function branch(name: string, overrides: Partial<GitBranch> = {}): GitBranch {
  return {
    name,
    is_current: false,
    is_remote: name.startsWith('origin/'),
    last_commit_date: new Date(0),
    ...overrides,
  };
}

describe('resolveDefaultBranch', () => {
  it('returns null when there are no branches', () => {
    expect(resolveDefaultBranch([])).toBeNull();
  });

  it('defaults to origin/main when present', () => {
    const branches = [
      branch('main', { is_current: true }),
      branch('origin/main'),
      branch('origin/feature-x'),
    ];
    expect(resolveDefaultBranch(branches)).toBe('origin/main');
  });

  it('falls back to origin/master when origin/main is absent', () => {
    const branches = [
      branch('master', { is_current: true }),
      branch('origin/master'),
    ];
    expect(resolveDefaultBranch(branches)).toBe('origin/master');
  });

  it('prefers a valid configured default over origin/main', () => {
    const branches = [
      branch('origin/main'),
      branch('origin/release'),
      branch('develop', { is_current: true }),
    ];
    expect(resolveDefaultBranch(branches, 'origin/release')).toBe(
      'origin/release'
    );
  });

  it('ignores a configured default that no longer exists', () => {
    const branches = [branch('origin/main'), branch('develop')];
    expect(resolveDefaultBranch(branches, 'origin/gone')).toBe('origin/main');
  });

  it('falls back to the current branch when no preferred match exists', () => {
    const branches = [
      branch('develop'),
      branch('feature', { is_current: true }),
    ];
    expect(resolveDefaultBranch(branches)).toBe('feature');
  });

  it('falls back to the first branch when nothing else matches', () => {
    const branches = [branch('develop'), branch('feature')];
    expect(resolveDefaultBranch(branches)).toBe('develop');
  });

  it('ignores a null/empty configured default', () => {
    const branches = [branch('origin/main'), branch('develop')];
    expect(resolveDefaultBranch(branches, null)).toBe('origin/main');
    expect(resolveDefaultBranch(branches, '')).toBe('origin/main');
  });
});
