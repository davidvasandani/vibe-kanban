import { describe, expect, it } from 'vitest';
import { shouldRenderWorkspaceContextBar } from './workspaceContextBarVisibility';

describe('shouldRenderWorkspaceContextBar', () => {
  it('hides when the responsive workspace layout is mobile', () => {
    expect(
      shouldRenderWorkspaceContextBar({
        isResponsiveMobile: true,
        isRealMobileDevice: false,
      })
    ).toBe(false);
  });

  it('hides when physical-device detection reports mobile', () => {
    expect(
      shouldRenderWorkspaceContextBar({
        isResponsiveMobile: false,
        isRealMobileDevice: true,
      })
    ).toBe(false);
  });

  it('hides when both mobile signals are true', () => {
    expect(
      shouldRenderWorkspaceContextBar({
        isResponsiveMobile: true,
        isRealMobileDevice: true,
      })
    ).toBe(false);
  });

  it('renders only when neither mobile signal is true', () => {
    expect(
      shouldRenderWorkspaceContextBar({
        isResponsiveMobile: false,
        isRealMobileDevice: false,
      })
    ).toBe(true);
  });
});
