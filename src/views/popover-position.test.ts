import { describe, it, expect } from 'vitest';
import { clampPopoverPosition } from './popover-position';

describe('clampPopoverPosition', () => {
  const viewport = { width: 1000, height: 800 };
  const popover = { width: 440, height: 200 };
  const anchor = { top: 300, bottom: 320 };

  it('keeps the position when the popover fits', () => {
    const pos = clampPopoverPosition({ x: 100, y: 326, anchor, popover, viewport });
    expect(pos).toEqual({ x: 100, y: 326 });
  });

  it('clamps x so the popover does not overflow the right edge', () => {
    const pos = clampPopoverPosition({ x: 900, y: 326, anchor, popover, viewport });
    expect(pos.x).toBe(1000 - 440 - 12);
  });

  it('never pushes x past the left edge', () => {
    const narrow = { width: 300, height: 800 };
    const pos = clampPopoverPosition({ x: 200, y: 326, anchor, popover, viewport: narrow });
    expect(pos.x).toBe(12);
  });

  it('flips above the anchor when it would overflow the bottom', () => {
    const lowAnchor = { top: 700, bottom: 720 };
    const pos = clampPopoverPosition({ x: 100, y: 726, anchor: lowAnchor, popover, viewport });
    expect(pos.y).toBe(700 - 200 - 6);
  });
});
