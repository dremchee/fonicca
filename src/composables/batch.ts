import type { Curve } from '@/stores/editor'
import { invoke } from '@tauri-apps/api/core'
import { shallowRef } from 'vue'

/** What to do to each file; mirrors `batch::Recipe`. */
export interface Recipe {
  trim: { thresholdDb: number, padMs: number } | null
  normalize: number | null
  lufs: { target: number, ceiling: number } | null
  dcOffset: boolean
  fadeInMs: number | null
  fadeOutMs: number | null
  curve: Curve
  channels: number | null
  sampleRate: number | null
  bitDepth: number | null
  rename: string | null
}

export type Destination = { mode: 'folder', path: string } | { mode: 'inPlace' }

export interface BatchProgress { done: number, total: number, name: string }
export interface BatchLoudness {
  name: string
  path: string
  target: number
  before: number
  after: number
  gainDb: number
  limited: boolean
}
export interface BatchDone {
  written: number
  loudness: BatchLoudness[]
  failed: { name: string, error: string }[]
  cancelled: boolean
  dir: string | null
}

export const BATCH_PROGRESS = 'batch:progress'
export const BATCH_DONE = 'batch:done'

export const batchApi = {
  preview: (ids: number[], template: string, total: number) => invoke<string[]>('batch_preview', { ids, template, total }),
  run: (ids: number[], recipe: Recipe, destination: Destination) => invoke<void>('batch_run', { ids, recipe, destination }),
  cancel: () => invoke<void>('batch_cancel'),
}

/** The samples the batch dialog works on; rendered by BatchDialog in App. */
export const batchFor = shallowRef<number[] | null>(null)

export function openBatch(ids: number[]) {
  if (ids.length)
    batchFor.value = ids
}
