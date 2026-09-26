import { describe, expect, it } from 'vitest';

import { formatIoPressure, isBlockedTaskSaturated, NO_READING } from './format';

describe('isBlockedTaskSaturated', () => {
  it('flags a node whose blocked tasks reach its core count', () => {
    expect(isBlockedTaskSaturated(6, 6)).toBe(true);
    expect(isBlockedTaskSaturated(9, 6)).toBe(true);
    expect(isBlockedTaskSaturated(5, 6)).toBe(false);
  });

  it('never flags an absent reading', () => {
    expect(isBlockedTaskSaturated(null, 6)).toBe(false);
    expect(isBlockedTaskSaturated(undefined, 6)).toBe(false);
    expect(isBlockedTaskSaturated(8, null)).toBe(false);
    expect(isBlockedTaskSaturated(8, 0)).toBe(false);
  });
});

describe('formatIoPressure', () => {
  it('renders some and full averages', () => {
    expect(formatIoPressure(2.17, 0.86)).toBe('2.2% / 0.9%');
  });

  it('renders absent readings as no reading, never 0', () => {
    expect(formatIoPressure(null, undefined)).toBe(
      `${NO_READING} / ${NO_READING}`
    );
  });
});
