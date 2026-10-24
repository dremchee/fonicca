import type { Recipe } from '@/composables/batch'
import type { Curve } from '@/stores/editor'

/** Persisted controls, including values for disabled steps. */
export interface BatchForm {
  trim: boolean
  thresholdDb: number
  padMs: number
  dcOffset: boolean
  normalize: boolean
  normalizeMode: 'peak' | 'lufs'
  peakDb: number
  targetLufs: number
  ceilingDb: number
  fades: boolean
  fadeInMs: number
  fadeOutMs: number
  curve: Curve
  format: boolean
  channels: string
  sampleRate: string
  bitDepth: string
  rename: boolean
  template: string
}

export const batchDefaults: BatchForm = {
  trim: true, thresholdDb: -60, padMs: 5, dcOffset: false,
  normalize: false, normalizeMode: 'peak', peakDb: -0.3, targetLufs: -14, ceilingDb: -0.3,
  fades: false, fadeInMs: 0, fadeOutMs: 10, curve: 'smooth',
  format: false, channels: 'keep', sampleRate: 'keep', bitDepth: 'keep',
  rename: false, template: '{category}_{bpm}_{key}_{n}',
}

export function restoreBatchForm(saved: Partial<BatchForm>): BatchForm {
  return { ...batchDefaults, ...saved }
}

export function validBatchForm(form: BatchForm) {
  const within = (v: number, min: number, max: number) => typeof v === 'number' && Number.isFinite(v) && v >= min && v <= max
  if (!form.normalize)
    return true
  return form.normalizeMode === 'peak' ? within(form.peakDb, -96, 0)
    : form.normalizeMode === 'lufs' && within(form.targetLufs, -36, -5) && within(form.ceilingDb, -12, 0)
}

export function batchRecipe(form: BatchForm, inPlace: boolean): Recipe {
  const opt = (v: string) => form.format && v !== 'keep' ? Number(v) : null
  return {
    trim: form.trim ? { thresholdDb: form.thresholdDb, padMs: form.padMs } : null,
    dcOffset: form.dcOffset,
    normalize: form.normalize && form.normalizeMode === 'peak' ? form.peakDb : null,
    lufs: form.normalize && form.normalizeMode === 'lufs' ? { target: form.targetLufs, ceiling: form.ceilingDb } : null,
    fadeInMs: form.fades ? form.fadeInMs : null,
    fadeOutMs: form.fades ? form.fadeOutMs : null,
    curve: form.curve,
    channels: opt(form.channels), sampleRate: opt(form.sampleRate), bitDepth: opt(form.bitDepth),
    rename: form.rename && !inPlace ? form.template : null,
  }
}
