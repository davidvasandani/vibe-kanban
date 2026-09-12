import { describe, expect, it } from 'vitest';
import { getReconnectDelay } from './useJsonPatchWsStream';

describe('getReconnectDelay', () => {
  it('uses bounded exponential backoff with jitter', () => {
    expect(getReconnectDelay(0, () => 0)).toBe(800);
    expect(getReconnectDelay(0, () => 0.5)).toBe(1_000);
    expect(getReconnectDelay(1, () => 1)).toBe(2_400);
    expect(getReconnectDelay(2, () => 0.5)).toBe(4_000);
    expect(getReconnectDelay(20, () => 1)).toBe(8_000);
  });

  it('clamps an invalid random source to the jitter bounds', () => {
    expect(getReconnectDelay(0, () => -10)).toBe(800);
    expect(getReconnectDelay(0, () => 10)).toBe(1_200);
  });
});
