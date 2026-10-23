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
  favorite: boolean
  tags: TagRef[]
}

export interface TagRef {
  id: number
  name: string
}

export interface Tag extends TagRef {
  sampleCount: number
}

export interface Collection {
  id: number
  name: string
  sampleCount: number
}

export interface ImportReport {
  matched: number
  unmatched: number
}

export interface ScanProgress {
  sourceId: number
  processed: number
  total: number
  done: boolean
  /** Rows added, updated or removed; set on the final event. */
  changes: number
  error: string | null
}

export interface SampleFilter {
  query?: string
  sourceId?: number | null
  tagId?: number | null
  favorites?: boolean
  collectionId?: number | null
}

export const SCAN_PROGRESS_EVENT = 'scan-progress'

export const api = {
  listSources: () => invoke<Source[]>('list_sources'),
  addSource: (path: string) => invoke<number>('add_source', { path }),
  removeSource: (id: number) => invoke<void>('remove_source', { id }),
  rescanSource: (id: number) => invoke<void>('rescan_source', { id }),
  countSamples: (filter: SampleFilter) => invoke<number>('count_samples', { filter }),
  listSamples: (filter: SampleFilter, offset: number, limit: number) =>
    invoke<Sample[]>('list_samples', { filter, offset, limit }),
  sampleIds: (filter: SampleFilter) => invoke<number[]>('sample_ids', { filter }),
  samplePaths: (ids: number[]) => invoke<string[]>('sample_paths', { ids }),
  setFavorite: (ids: number[], favorite: boolean) => invoke<void>('set_favorite', { ids, favorite }),
  listTags: () => invoke<Tag[]>('list_tags'),
  tagSamples: (ids: number[], name: string) => invoke<TagRef>('tag_samples', { ids, name }),
  untagSamples: (ids: number[], tagId: number) => invoke<void>('untag_samples', { ids, tagId }),
  renameTag: (id: number, name: string) => invoke<void>('rename_tag', { id, name }),
  deleteTag: (id: number) => invoke<void>('delete_tag', { id }),
  listCollections: () => invoke<Collection[]>('list_collections'),
  createCollection: (name: string, ids: number[] = []) => invoke<number>('create_collection', { name, ids }),
  renameCollection: (id: number, name: string) => invoke<void>('rename_collection', { id, name }),
  deleteCollection: (id: number) => invoke<void>('delete_collection', { id }),
  addToCollection: (ids: number[], collectionId: number) => invoke<void>('add_to_collection', { ids, collectionId }),
  removeFromCollection: (ids: number[], collectionId: number) =>
    invoke<void>('remove_from_collection', { ids, collectionId }),
  exportUserData: (path: string) => invoke<number>('export_user_data', { path }),
  importUserData: (path: string) => invoke<ImportReport>('import_user_data', { path }),
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
