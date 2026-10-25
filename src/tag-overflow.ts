/** Keep a prefix of complete badges, reserving space for the overflow control. */
export function visibleTagCount(widths: readonly number[], available: number, counterWidth: number, gap = 4) {
  const all = widths.reduce((sum, width) => sum + width, 0) + Math.max(0, widths.length - 1) * gap
  if (all <= available)
    return widths.length
  let used = 0
  let count = 0
  for (const width of widths) {
    const next = used + (count ? gap : 0) + width
    if (next + gap + counterWidth > available)
      break
    used = next
    count++
  }
  return count
}
