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

export function tagColorStyle(name: string) {
  let hash = 2166136261
  for (const char of name.normalize('NFKC').trim().toLowerCase())
    hash = Math.imul(hash ^ char.codePointAt(0)!, 16777619) >>> 0
  const [lightBg, lightInk, darkBg, darkInk] = TAG_COLORS[hash % TAG_COLORS.length]
  return { '--tag-light-bg': lightBg, '--tag-light-ink': lightInk, '--tag-dark-bg': darkBg, '--tag-dark-ink': darkInk }
}
