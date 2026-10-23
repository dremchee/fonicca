import { t } from '@/i18n'

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds))
    return '—'
  if (seconds < 10)
    return t('common.units.seconds', { n: seconds.toFixed(2) })
  const m = Math.floor(seconds / 60)
  const s = Math.floor(seconds % 60)
  return `${m}:${String(s).padStart(2, '0')}`
}

export function formatRate(hz: number | null | undefined): string {
  if (!hz)
    return '—'
  return t('common.units.kHz', { n: (hz / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 }) })
}

export function formatChannels(n: number | null | undefined): string {
  if (!n)
    return '—'
  return n === 1 ? t('common.channels.mono') : n === 2 ? t('common.channels.stereo') : t('common.channels.many', { n })
}

export function formatSize(bytes: number): string {
  if (bytes < 1024)
    return t('common.units.bytes', { n: bytes })
  const units = ['KB', 'MB', 'GB'] as const
  let v = bytes / 1024
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return t(`common.units.${units[i]!}`, { n: v.toFixed(v < 10 ? 1 : 0) })
}

export function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path
}

export function formatBpm(bpm: number | null | undefined): string {
  if (bpm == null)
    return '—'
  return Number.isInteger(bpm) ? String(bpm) : bpm.toFixed(1)
}

export function formatDb(db: number | null | undefined, unit: 'dB' | 'LUFS' = 'dB'): string {
  if (db == null || !Number.isFinite(db))
    return '—'
  return `${db.toFixed(1)} ${t(`common.units.${unit}`)}`
}

export const KINDS = ['oneshot', 'loop', 'long'] as const
export type Kind = typeof KINDS[number]

/** Category ids in the same order as `category::CATEGORIES` in Rust. */
export const CATEGORIES = [
  'kick', 'snare', 'clap', 'hihat', 'cymbal', 'tom', 'percussion', 'drums',
  'bass', 'synth', 'pad', 'keys', 'guitar', 'strings', 'brass', 'vocal', 'fx',
] as const
export type Category = typeof CATEGORIES[number]

export function kindLabel(kind: string, plural = false): string {
  return (KINDS as readonly string[]).includes(kind) ? t(`common.${plural ? 'kindsPlural' : 'kinds'}.${kind as Kind}`) : kind
}

/** Display name for a category id; unknown ids show as they are. */
export function categoryLabel(id: string): string {
  return (CATEGORIES as readonly string[]).includes(id) ? t(`common.categories.${id as Category}`) : id
}

export function sourceHint(source: 'name' | 'audio' | null | undefined): string {
  return source === 'name' ? t('common.hints.fromName') : source === 'audio' ? t('common.hints.fromAudio') : ''
}

export function categoryHint(source: 'name' | 'audio' | null | undefined, confidence: number | null | undefined): string {
  if (source === 'audio' && confidence != null)
    return t('common.hints.categoryFromAudio', { percent: Math.round(confidence * 100) })
  return source === 'name' ? t('common.hints.categoryFromName') : ''
}

/**
 * Breadcrumb-style short path: home becomes "~" and the middle folders "…",
 * keeping the first two folders, the file's folder and the file:
 * "/Users/me/Projects/Sound/Beat/Kit/007.wav" → "~/Projects/…/Kit/007.wav".
 */
export function shortPath(path: string, keepStart = 2, keepEnd = 2): string {
  const home = path.match(/^\/Users\/[^/]+/)?.[0]
  const rest = home ? path.slice(home.length) : path
  const parts = rest.split('/').filter(Boolean)
  const prefix = home ? '~/' : '/'
  if (parts.length <= keepStart + keepEnd + 1)
    return prefix + parts.join('/')
  return `${prefix}${[...parts.slice(0, keepStart), '…', ...parts.slice(-keepEnd)].join('/')}`
}

