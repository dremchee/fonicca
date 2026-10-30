import { shallowRef } from 'vue'
import type { AudioExportOptions } from '@/audio-export'

interface ExportChoice extends AudioExportOptions {
  sourceRate: number
  resolve: (options: AudioExportOptions | null) => void
}
export const pendingAudioExport = shallowRef<ExportChoice | null>(null)

export function chooseAudioExport(options: AudioExportOptions, sourceRate: number): Promise<AudioExportOptions | null> {
  settleAudioExport(null)
  return new Promise(resolve => { pendingAudioExport.value = { ...options, sourceRate, resolve } })
}

export function settleAudioExport(options: AudioExportOptions | null) {
  const pending = pendingAudioExport.value
  pendingAudioExport.value = null
  pending?.resolve(options)
}
