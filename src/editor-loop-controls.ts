export const LOOP_FLAG_SIZE = 7
export const LOOP_FLAG_WIDTH = 7
export const LOOP_LABEL_WIDTH = 36
export const LOOP_LABEL_HEIGHT = 14

/** Shared by canvas drawing and hit testing, including a partially visible loop. */
export function loopLabelBox(start: number, end: number, width: number, ruler: number, hasRegionBadge = false) {
  const left = Math.max(0, start) + (hasRegionBadge ? 24 : 4)
  const right = Math.min(width, end) - 4
  return right - left >= LOOP_LABEL_WIDTH
    ? { x: left, y: ruler + 3, width: LOOP_LABEL_WIDTH, height: LOOP_LABEL_HEIGHT }
    : null
}

export function loopBoundaryAt(x: number, y: number, start: number, end: number, _ruler: number, tolerance = 5): 'start' | 'end' | null {
  const a = Math.abs(x - start), b = Math.abs(x - end)
  if (Math.min(a, b) <= tolerance) return a < b ? 'start' : 'end'
  if (y >= 0 && y <= LOOP_FLAG_SIZE + 2) {
    if (x >= start && x <= start + LOOP_FLAG_WIDTH + 2) return 'start'
    if (x <= end && x >= end - LOOP_FLAG_WIDTH - 2) return 'end'
  }
  return null
}
