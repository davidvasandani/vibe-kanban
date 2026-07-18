import { describe, expect, it } from 'vitest';
import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import { carouselTier, needsFeedback, sortForCarousel } from './carouselSort';

let counter = 0;
function ws(overrides: Partial<SidebarWorkspace>): SidebarWorkspace {
  counter += 1;
  return {
    id: `ws-${counter}`,
    name: `Workspace ${counter}`,
    branch: `branch-${counter}`,
    createdAt: '2026-07-01T00:00:00Z',
    updatedAt: '2026-07-01T00:00:00Z',
    description: '',
    ...overrides,
  };
}

describe('needsFeedback', () => {
  it('is true for a pending approval even while running', () => {
    expect(
      needsFeedback(ws({ hasPendingApproval: true, isRunning: true }))
    ).toBe(true);
  });

  it('is true for unseen activity while not running', () => {
    expect(
      needsFeedback(ws({ hasUnseenActivity: true, isRunning: false }))
    ).toBe(true);
  });

  it('is false for unseen activity while still running', () => {
    expect(
      needsFeedback(ws({ hasUnseenActivity: true, isRunning: true }))
    ).toBe(false);
  });

  it('is false for a quiet idle workspace', () => {
    expect(needsFeedback(ws({ isRunning: false }))).toBe(false);
  });
});

describe('carouselTier', () => {
  it('puts needs-feedback in tier 0', () => {
    expect(carouselTier(ws({ hasPendingApproval: true }))).toBe(0);
    expect(
      carouselTier(ws({ hasUnseenActivity: true, isRunning: false }))
    ).toBe(0);
  });

  it('puts seen failed/killed/interrupted runs in tier 1', () => {
    expect(carouselTier(ws({ latestProcessStatus: 'failed' }))).toBe(1);
    expect(carouselTier(ws({ latestProcessStatus: 'killed' }))).toBe(1);
    expect(carouselTier(ws({ latestProcessStatus: 'interrupted' }))).toBe(1);
  });

  it('sorts an interrupted workspace ahead of idle and running ones', () => {
    const running = ws({ isRunning: true });
    const idle = ws({ isRunning: false });
    const interrupted = ws({ latestProcessStatus: 'interrupted' });

    const sorted = sortForCarousel(
      [running, idle, interrupted],
      'needs_feedback'
    );
    expect(sorted.map((w) => w.id)).toEqual([
      interrupted.id,
      idle.id,
      running.id,
    ]);
  });

  it('treats an unseen failure as needing feedback, not tier 1', () => {
    expect(
      carouselTier(
        ws({ latestProcessStatus: 'failed', hasUnseenActivity: true })
      )
    ).toBe(0);
  });

  it('puts idle in tier 2 and running in tier 3', () => {
    expect(carouselTier(ws({ isRunning: false }))).toBe(2);
    expect(
      carouselTier(ws({ isRunning: true, latestProcessStatus: 'running' }))
    ).toBe(3);
  });
});

describe('sortForCarousel needs_feedback mode', () => {
  it('orders tiers left to right: feedback, failed, idle, running', () => {
    const running = ws({ isRunning: true });
    const idle = ws({ isRunning: false });
    const failed = ws({ latestProcessStatus: 'failed' });
    const unseen = ws({ hasUnseenActivity: true, isRunning: false });
    const approval = ws({ hasPendingApproval: true, isRunning: true });

    const sorted = sortForCarousel(
      [running, idle, failed, unseen, approval],
      'needs_feedback'
    );
    expect(sorted.map((w) => w.id)).toEqual([
      approval.id,
      unseen.id,
      failed.id,
      idle.id,
      running.id,
    ]);
  });

  it('puts pending approvals ahead of stopped-unseen within tier 0', () => {
    const unseen = ws({
      hasUnseenActivity: true,
      isRunning: false,
      updatedAt: '2026-07-02T00:00:00Z',
    });
    const approval = ws({
      hasPendingApproval: true,
      updatedAt: '2026-07-01T00:00:00Z',
    });

    const sorted = sortForCarousel([unseen, approval], 'needs_feedback');
    expect(sorted.map((w) => w.id)).toEqual([approval.id, unseen.id]);
  });

  it('breaks ties by most recent activity', () => {
    const older = ws({
      hasUnseenActivity: true,
      isRunning: false,
      latestProcessCompletedAt: '2026-07-01T00:00:00Z',
    });
    const newer = ws({
      hasUnseenActivity: true,
      isRunning: false,
      latestProcessCompletedAt: '2026-07-03T00:00:00Z',
    });

    const sorted = sortForCarousel([older, newer], 'needs_feedback');
    expect(sorted.map((w) => w.id)).toEqual([newer.id, older.id]);
  });

  it('ignores pinning: a pinned running workspace stays right of feedback', () => {
    const pinnedRunning = ws({ isPinned: true, isRunning: true });
    const unseen = ws({ hasUnseenActivity: true, isRunning: false });

    const sorted = sortForCarousel([pinnedRunning, unseen], 'needs_feedback');
    expect(sorted.map((w) => w.id)).toEqual([unseen.id, pinnedRunning.id]);
  });
});

describe('sortForCarousel other modes', () => {
  it('sorts by updated_at desc with pinned first', () => {
    const older = ws({ updatedAt: '2026-07-01T00:00:00Z' });
    const newer = ws({ updatedAt: '2026-07-05T00:00:00Z' });
    const pinnedOldest = ws({
      isPinned: true,
      updatedAt: '2026-06-01T00:00:00Z',
    });

    const sorted = sortForCarousel([older, newer, pinnedOldest], 'updated_at');
    expect(sorted.map((w) => w.id)).toEqual([
      pinnedOldest.id,
      newer.id,
      older.id,
    ]);
  });

  it('sorts by created_at desc', () => {
    const older = ws({ createdAt: '2026-07-01T00:00:00Z' });
    const newer = ws({ createdAt: '2026-07-05T00:00:00Z' });

    const sorted = sortForCarousel([older, newer], 'created_at');
    expect(sorted.map((w) => w.id)).toEqual([newer.id, older.id]);
  });

  it('sorts by name', () => {
    const b = ws({ name: 'bravo' });
    const a = ws({ name: 'alpha' });

    const sorted = sortForCarousel([b, a], 'name');
    expect(sorted.map((w) => w.id)).toEqual([a.id, b.id]);
  });

  it('does not mutate its input', () => {
    const first = ws({ name: 'bravo' });
    const second = ws({ name: 'alpha' });
    const input = [first, second];

    sortForCarousel(input, 'name');
    expect(input.map((w) => w.id)).toEqual([first.id, second.id]);
  });
});
