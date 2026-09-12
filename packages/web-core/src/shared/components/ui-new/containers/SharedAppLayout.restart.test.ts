import { describe, expect, it } from 'vitest';
import { shouldShowRestartBanner } from './restartVisibility';

describe('shouldShowRestartBanner', () => {
  it('waits for the initial workspace snapshot before calling it a restart', () => {
    expect(shouldShowRestartBanner(true, false)).toBe(false);
  });

  it('shows during a post-initialization disconnect and clears on recovery', () => {
    expect(shouldShowRestartBanner(false, false)).toBe(true);
    expect(shouldShowRestartBanner(false, true)).toBe(false);
  });
});
