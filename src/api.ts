import { Channel, invoke } from '@tauri-apps/api/core'

export interface Source {
  id: number
  path: string
  sampleCount: number
}

export interface Sample {
  id: number
  sourceId: number
  path: string
  name: string
  ext: string
  size: number
  duration: number | null
  sampleRate: number | null
  channels: number | null
  bitDepth: number | null
}

export interface ScanProgress {
  sourceId: number
  processed: number
  total: number
  done: boolean
  error: string | null
}

export interface SampleFilter {
  query?: string
  sourceId?: number | null
}

export const SCAN_PROGRESS_EVENT = 'scan-progress'

export const api = {
  listSources: () => invoke<Source[]>('list_sources'),
  addSource: (path: string) => invoke<number>('add_source', { path }),
  removeSource: (id: number) => invoke<void>('remove_source', { id }),
  rescanSource: (id: number) => invoke<void>('rescan_source', { id }),
  countSamples: (f: SampleFilter) =>
    invoke<number>('count_samples', { query: f.query || null, sourceId: f.sourceId ?? null }),
  listSamples: (f: SampleFilter, offset: number, limit: number) =>
    invoke<Sample[]>('list_samples', { query: f.query || null, sourceId: f.sourceId ?? null, offset, limit }),
  getPeaks: (id: number) => invoke<number[]>('get_peaks', { id }),
  play: (id: number, start = 0) => invoke<void>('play', { id, start }),
  stop: () => invoke<void>('stop'),
  seek: (position: number) => invoke<void>('seek', { position }),
  playbackPosition: () => invoke<number | null>('playback_position'),
}

let dragIcon: string | undefined

/** Small PNG shown under the cursor while dragging files out of the app. */
function getDragIcon(): string {
  if (dragIcon)
    return dragIcon
  const c = document.createElement('canvas')
  c.width = c.height = 32
  const ctx = c.getContext('2d')!
  ctx.fillStyle = '#7c5cff'
  ctx.beginPath()
  ctx.roundRect(2, 2, 28, 28, 6)
  ctx.fill()
  ctx.fillStyle = '#fff'
  for (const [x, h] of [[8, 8], [13, 16], [18, 12], [23, 6]])
    ctx.fillRect(x, 16 - h / 2, 2, h)
  dragIcon = c.toDataURL('image/png')
  return dragIcon
}

/** Starts a native OS drag of the given files (e.g. into a DAW). */
export function startFileDrag(paths: string[]) {
  return invoke('plugin:drag|start_drag', {
    item: paths,
    image: getDragIcon(),
    options: { mode: 'copy' },
    onEvent: new Channel(),
  })
}
