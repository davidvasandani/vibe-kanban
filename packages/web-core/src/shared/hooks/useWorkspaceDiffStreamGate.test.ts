import { describe, expect, it } from 'vitest';
import { wantsWorkspaceDiffStream } from './useWorkspaceDiffStreamGate';

describe('wantsWorkspaceDiffStream', () => {
  it('opens immediately on desktop', () => {
    expect(
      wantsWorkspaceDiffStream({
        isMobile: false,
        mobileTab: 'chat',
        chatHistorySettled: false,
      })
    ).toBe(true);
  });

  it('keeps the phone chat from queueing behind the diff socket', () => {
    expect(
      wantsWorkspaceDiffStream({
        isMobile: true,
        mobileTab: 'chat',
        chatHistorySettled: false,
      })
    ).toBe(false);
  });

  it('opens on mobile once the chat has rendered or a diff tab is shown', () => {
    expect(
      wantsWorkspaceDiffStream({
        isMobile: true,
        mobileTab: 'chat',
        chatHistorySettled: true,
      })
    ).toBe(true);
    for (const mobileTab of ['changes', 'git']) {
      expect(
        wantsWorkspaceDiffStream({
          isMobile: true,
          mobileTab,
          chatHistorySettled: false,
        })
      ).toBe(true);
    }
  });
});
