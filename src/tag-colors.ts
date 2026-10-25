/** Quiet backgrounds with readable foregrounds in both app appearances. */
export const TAG_COLORS = [
  ['#fee2e2', '#991b1b', '#451a20', '#fecaca'],
  ['#ffedd5', '#9a3412', '#431f12', '#fed7aa'],
  ['#fef3c7', '#854d0e', '#392b10', '#fde68a'],
  ['#dcfce7', '#166534', '#123322', '#bbf7d0'],
  ['#ccfbf1', '#115e59', '#103331', '#99f6e4'],
  ['#dbeafe', '#1e40af', '#172a46', '#bfdbfe'],
  ['#ede9fe', '#5b21b6', '#2d204b', '#ddd6fe'],
  ['#fce7f3', '#9d174d', '#431d34', '#fbcfe8'],
] as const

export function tagColorStyle(name: string, customColor?: string) {
  let hash = 2166136261
  for (const char of name.normalize('NFKC').trim().toLowerCase())
    hash = Math.imul(hash ^ char.codePointAt(0)!, 16777619) >>> 0
  const [lightBg, lightInk, darkBg, darkInk] = TAG_COLORS[hash % TAG_COLORS.length]
  if (customColor && /^#[0-9a-f]{6}$/i.test(customColor)) {
    const rgb = [1, 3, 5].map(at => parseInt(customColor.slice(at, at + 2), 16))
    const blend = (target: number, amount: number) => `#${rgb.map(c => Math.round(c * (1 - amount) + target * amount).toString(16).padStart(2, '0')).join('')}`
    return {
      '--tag-light-bg': blend(255, 0.88), '--tag-light-ink': blend(0, 0.65),
      '--tag-dark-bg': blend(0, 0.85), '--tag-dark-ink': blend(255, 0.7),
      '--tag-marker': customColor,
    }
  }
  return { '--tag-light-bg': lightBg, '--tag-light-ink': lightInk, '--tag-dark-bg': darkBg, '--tag-dark-ink': darkInk }
}
