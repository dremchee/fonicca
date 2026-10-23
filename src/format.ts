export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds))
    return '—'
  if (seconds < 10)
    return `${seconds.toFixed(2)}s`
  const m = Math.floor(seconds / 60)
  const s = Math.floor(seconds % 60)
  return `${m}:${String(s).padStart(2, '0')}`
}

export function formatRate(hz: number | null | undefined): string {
  if (!hz)
    return '—'
  return `${(hz / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 })} kHz`
}

export function formatChannels(n: number | null | undefined): string {
  if (!n)
    return '—'
  return n === 1 ? 'Mono' : n === 2 ? 'Stereo' : `${n} ch`
}

export function formatSize(bytes: number): string {
  if (bytes < 1024)
    return `${bytes} B`
  const units = ['KB', 'MB', 'GB']
  let v = bytes / 1024
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`
}

export function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path
}

export function formatBpm(bpm: number | null | undefined): string {
  if (bpm == null)
    return '—'
  return Number.isInteger(bpm) ? String(bpm) : bpm.toFixed(1)
}

export function formatDb(db: number | null | undefined, unit = 'dB'): string {
  if (db == null || !Number.isFinite(db))
    return '—'
  return `${db.toFixed(1)} ${unit}`
}

export const KIND_LABELS = { oneshot: 'One-shot', loop: 'Loop', long: 'Long' } as const

/** Display names for category ids (same order as `category::CATEGORIES` in Rust). */
export const CATEGORY_LABELS: Record<string, string> = {
  kick: 'Kick',
  snare: 'Snare',
  clap: 'Clap',
  hihat: 'Hi-hat',
  cymbal: 'Cymbal',
  tom: 'Tom',
  percussion: 'Percussion',
  drums: 'Drums',
  bass: 'Bass',
  synth: 'Synth',
  pad: 'Pad',
  keys: 'Keys',
  guitar: 'Guitar',
  strings: 'Strings',
  brass: 'Brass & winds',
  vocal: 'Vocal',
  fx: 'FX',
}

export function sourceHint(source: 'name' | 'audio' | null | undefined): string {
  return source === 'name' ? 'From the file name' : source === 'audio' ? 'Detected from audio' : ''
}

export function categoryHint(source: 'name' | 'audio' | null | undefined, confidence: number | null | undefined): string {
  if (source === 'audio' && confidence != null)
    return `Detected from audio (${Math.round(confidence * 100)}% of similar sounds agree)`
  return source === 'name' ? 'From the file or folder name' : ''
}
