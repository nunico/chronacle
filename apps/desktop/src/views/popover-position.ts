/** Clamp a fixed-position popover so it stays inside the viewport.
 *
 * The popover is initially placed below its anchor (the citation badge).
 * If it would overflow the right edge it is shifted left; if it would
 * overflow the bottom it flips above the anchor.
 */

const EDGE_MARGIN_PX = 12;
const ANCHOR_GAP_PX = 6;

export interface ClampInput {
  x: number;
  y: number;
  anchor: { top: number; bottom: number };
  popover: { width: number; height: number };
  viewport: { width: number; height: number };
}

export function clampPopoverPosition({ x, y, anchor, popover, viewport }: ClampInput): {
  x: number;
  y: number;
} {
  let cx = Math.min(x, viewport.width - popover.width - EDGE_MARGIN_PX);
  cx = Math.max(cx, EDGE_MARGIN_PX);

  let cy = y;
  if (y + popover.height > viewport.height - EDGE_MARGIN_PX) {
    cy = anchor.top - popover.height - ANCHOR_GAP_PX;
  }

  return { x: cx, y: cy };
}
