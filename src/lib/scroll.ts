/** Whether a scroll container is at (or near) its bottom edge.
 *
 * Used to decide if the chat thread should auto-follow streaming tokens:
 * we only force-scroll while the user is already reading the latest
 * message, never while they have scrolled up.
 */

const NEAR_BOTTOM_TOLERANCE_PX = 60;

export interface ScrollMetrics {
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
}

export function isNearBottom({ scrollTop, clientHeight, scrollHeight }: ScrollMetrics): boolean {
  return scrollHeight - (scrollTop + clientHeight) <= NEAR_BOTTOM_TOLERANCE_PX;
}
