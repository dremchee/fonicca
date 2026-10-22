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
