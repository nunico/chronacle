import { describe, it, expect } from 'vitest';
import { isNearBottom } from './scroll';

describe('isNearBottom', () => {
  it('is true when scrolled exactly to the bottom', () => {
    expect(isNearBottom({ scrollTop: 900, clientHeight: 100, scrollHeight: 1000 })).toBe(true);
  });

  it('is true within the tolerance band above the bottom', () => {
    expect(isNearBottom({ scrollTop: 850, clientHeight: 100, scrollHeight: 1000 })).toBe(true);
  });

  it('is false when scrolled well above the bottom', () => {
    expect(isNearBottom({ scrollTop: 200, clientHeight: 100, scrollHeight: 1000 })).toBe(false);
  });

  it('is true for content shorter than the viewport', () => {
    expect(isNearBottom({ scrollTop: 0, clientHeight: 500, scrollHeight: 300 })).toBe(true);
  });
});
