import type { Locale } from '@/i18n'
import type { CustomTheme } from '@/themes'
import { Channel, invoke } from '@tauri-apps/api/core'

export interface Source {
  id: number
  path: string
  sampleCount: number
}

export interface SourceFolder {
  path: string
  sampleCount: number
}

/** Tags read from the file itself (ID3, Vorbis, MP4…); most samples have none. */
export interface TrackTags {
  title: string | null
  artist: string | null
  album: string | null
  year: number | null
  genre: string | null
  hasCover: boolean
}

export interface TagEdit {
  title: string | null
  artist: string | null
  album: string | null
  year: number | null
  genre: string | null
  cover: { kind: 'keep' } | { kind: 'remove' } | { kind: 'file', path: string }
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
  kind: SampleKind | null
  bpm: number | null
  bpmSource: ValueSource | null
  key: string | null
  keySource: ValueSource | null
  peakDb: number | null
  lufs: number | null
  analyzed: boolean
  category: string | null
  categorySource: ValueSource | null
  categoryConfidence: number | null
  /** Cosine similarity to the sample being matched in a "find similar" view. */
  similarity: number | null
}

export type SampleKind = 'oneshot' | 'loop' | 'long'
/** Whether a BPM/key value was read from the file name or detected from audio. */
export type ValueSource = 'name' | 'audio'

export interface AnalysisProgress {
  pending: number
  pendingEmbeddings: number
  modelReady: boolean
  paused: boolean
  updated: number[]
}

export interface SoundModelStatus {
  installed: boolean
  ready: boolean
  downloading: boolean
  size: number
}

export type StemModel = 'spleeter' | 'scnetLarge' | 'scnetXlIhf'

export interface ModelStorage {
  id: 'clap' | 'apollo' | StemModel
  path: string
  installed: boolean
  downloading: boolean
  /** Actual file sizes, including incomplete downloads and notices. */
  size: number
  downloadSize: number
}

export interface DownloadProgress {
  received: number
  total: number
  done: boolean
  error: string | null
}

export type Theme = 'system' | 'light' | 'dark'

export interface WaveformSettings {
  waveStyle: WaveStyle
  waveChannels: 'mix' | 'split'
  waveScale: 'linear' | 'db'
  waveColors: 'theme' | 'frequency'
  waveDensity: 'off' | 'gaussian' | 'laplace' | 'flat'
  wavePeakOpacity: number
  waveRmsOpacity: number
  waveThickness: number
  waveBarWidth: number
  waveBarGap: number
  waveBarRadius: number
  waveDbRange: number
  waveChannelColors: [string | null, string | null]
}

export interface Settings extends WaveformSettings {
  theme: Theme
  /** Colour theme id: built-in or one of `customThemes`. */
  colorTheme: string
  customThemes: CustomTheme[]
  /** Interface language. */
  language: Locale
  watchFolders: boolean
  /** Play a sample when the focus moves to it with the arrow keys. */
  autoplay: boolean
  /** Preview volume, 0..1. */
  volume: number
  /** Analysis threads; null picks half the cores. */
  analysisThreads: number | null
  /** Preview loops in the project tempo and tonal samples in the project key. */
  syncEnabled: boolean
  projectBpm: number | null
  /** Key name like "Am" or "F#". */
  projectKey: string | null
  editorWaveform: WaveformSettings
}

export type WaveStyle = 'peaks' | 'peaksRms' | 'bars'

/** How playback changed a sample to fit the project. */
export interface Adjust {
  /** Output length ÷ input length (2 = half speed). */
  stretch: number
  semitones: number
}

export interface AppInfo {
  version: string
  cores: number
  defaultThreads: number
  dbPath: string
  dbSize: number
  modelsDir: string
  modelsSize: number
  settingsPath: string
}

export interface DuplicateCopy extends Sample {
  /** Modification time, Unix seconds. */
  mtime: number
}

export interface DuplicateGroup {
  key: string
  size: number
  /** False while large copies wait for their full-content hash; such groups can be hidden but not trashed. */
  verified: boolean
  copies: DuplicateCopy[]
}

export interface DuplicateResolution {
  keep: number
  extras: number[]
}

export type DuplicateAction = 'trash' | 'hide'

export interface ResolveReport {
  removed: number
  bytes: number
  skipped: string[]
}

export interface HiddenFile {
  path: string
  sourceId: number
  hiddenAt: number
}

/** Levels since the previous read, linear (1 = 0 dBFS), one per channel. */
export interface PlaybackLevels {
  peak: number[]
  rms: number[]
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
  directory?: string | null
  tagId?: number | null
  favorites?: boolean
  collectionId?: number | null
  kind?: SampleKind | null
  bpmMin?: number | null
  bpmMax?: number | null
  key?: string | null
  category?: string | null
  /** Show samples that sound like this one, most similar first. */
  similarTo?: number | null
}

export const SCAN_PROGRESS_EVENT = 'scan-progress'
export const ANALYSIS_PROGRESS_EVENT = 'analysis-progress'
export const MODEL_DOWNLOAD_EVENT = 'model-download'
export const DUPLICATES_VERIFIED_EVENT = 'duplicates-verified'

export const api = {
  listSources: () => invoke<Source[]>('list_sources'),
  sourceFolders: (sourceId: number) => invoke<SourceFolder[]>('source_folders', { sourceId }),
  addSource: (path: string) => invoke<number>('add_source', { path }),
  removeSource: (id: number) => invoke<void>('remove_source', { id }),
  rescanSource: (id: number) => invoke<void>('rescan_source', { id }),
  countSamples: (filter: SampleFilter) => invoke<number>('count_samples', { filter }),
  listSamples: (filter: SampleFilter, offset: number, limit: number) =>
    invoke<Sample[]>('list_samples', { filter, offset, limit }),
  sampleIds: (filter: SampleFilter) => invoke<number[]>('sample_ids', { filter }),
  samplePaths: (ids: number[]) => invoke<string[]>('sample_paths', { ids }),
  sampleTags: (id: number) => invoke<TrackTags | null>('sample_tags', { id }),
  /** Embedded cover art as image bytes, empty when there is none. */
  sampleCover: (id: number) => invoke<ArrayBuffer>('sample_cover', { id }),
  /** Bytes of an image file picked as a new cover, for a preview; rejects non-images. */
  coverPreview: (path: string) => invoke<ArrayBuffer>('cover_preview', { path }),
  /** Writes tags into the file; empty fields are removed. Returns the tags as read back. */
  updateSampleTags: (id: number, edit: TagEdit) => invoke<TrackTags | null>('update_sample_tags', { id, edit }),
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
  /** Whole-file waveform (min, max, RMS, bands per column), raw bytes for `parseWave`. */
  sampleWave: (id: number, columns: number, split: boolean) => invoke<ArrayBuffer>('sample_wave', { id, columns, split }),
  prioritizeAnalysis: (ids: number[]) => invoke<void>('prioritize_analysis', { ids }),
  setAnalysisPaused: (paused: boolean) => invoke<void>('set_analysis_paused', { paused }),
  analysisStatus: () => invoke<AnalysisProgress>('analysis_status'),
  soundModelStatus: () => invoke<SoundModelStatus>('sound_model_status'),
  modelStorage: () => invoke<ModelStorage[]>('model_storage'),
  downloadSoundModel: () => invoke<void>('download_sound_model'),
  deleteSoundModel: () => invoke<void>('delete_sound_model'),
  getSettings: () => invoke<Settings>('get_settings'),
  updateSettings: (settings: Settings) => invoke<Settings>('update_settings', { settings }),
  appInfo: () => invoke<AppInfo>('app_info'),
  exportTheme: (path: string, theme: CustomTheme) => invoke<void>('export_theme', { path, theme }),
  importTheme: (path: string) => invoke<CustomTheme>('import_theme', { path }),
  duplicateCount: () => invoke<number>('duplicate_count'),
  duplicateGroups: () => invoke<DuplicateGroup[]>('duplicate_groups'),
  verifyDuplicates: () => invoke<void>('verify_duplicates'),
  resolveDuplicates: (resolutions: DuplicateResolution[], action: DuplicateAction) =>
    invoke<ResolveReport>('resolve_duplicates', { resolutions, action }),
  listHidden: () => invoke<HiddenFile[]>('list_hidden'),
  /** All hidden files with `null`. */
  unhideSamples: (paths: string[] | null) => invoke<void>('unhide_samples', { paths }),
  preferredFolders: () => invoke<string[]>('preferred_folders'),
  setPreferredFolder: (path: string, preferred: boolean) => invoke<void>('set_preferred_folder', { path, preferred }),
  play: (id: number, start = 0, sync: { bpm: number | null, key: string | null } | null = null) =>
    invoke<Adjust>('play', { id, start, sync }),
  stop: () => invoke<void>('stop'),
  pause: () => invoke<number | null>('pause'),
  resume: () => invoke<boolean>('resume'),
  seek: (position: number) => invoke<void>('seek', { position }),
  playbackPosition: () => invoke<number | null>('playback_position'),
  /** `null` when nothing plays. */
  playbackLevels: () => invoke<PlaybackLevels | null>('playback_levels'),
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
