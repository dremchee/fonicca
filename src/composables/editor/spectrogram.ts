import type { Ref } from 'vue'
import type { Spectrogram } from '@/spectrogram'
import { onScopeDispose, shallowRef, watch } from 'vue'

interface Options {
  enabled: Ref<boolean>
  revision: Ref<number>
  frames: Ref<number>
  start: Ref<number>
  end: Ref<number>
  width: Ref<number>
  read: (start: number, end: number, columns: number) => Promise<Spectrogram>
}

/** One in-flight read and an overscanned viewport; edits cannot publish stale data. */
export function useEditorSpectrogram(options: Options) {
  const { enabled, revision, frames, start, end, width, read } = options
  const data = shallowRef<Spectrogram | null>(null)
  const loading = shallowRef(false)
  const error = shallowRef<string | null>(null)
  let dataRevision = -1
  let reading = false
  let again = false
  let disposed = false

  async function refresh() {
    if (dataRevision !== revision.value)
      data.value = null
    again = true
    if (reading || disposed)
      return
    reading = true
    try {
      while (again && !disposed) {
        again = false
        if (dataRevision !== revision.value)
          data.value = null
        if (!enabled.value || width.value <= 0 || frames.value <= 0)
          continue
        const span = Math.max(1, end.value - start.value)
        const current = data.value
        if (current && current.start <= start.value && current.end >= end.value
          && Math.abs((current.end - current.start) / current.columns / (span / Math.min(width.value, 1536)) - 1) < 0.25) {
          error.value = null
          continue
        }
        const wide = Math.min(frames.value, span * 1.5)
        const a = Math.floor(Math.max(0, Math.min(frames.value - wide, start.value - (wide - span) / 2)))
        const b = Math.min(frames.value, Math.ceil(a + wide))
        const columns = Math.max(1, Math.min(2048, Math.round(Math.min(width.value, 1536) * (b - a) / span)))
        const version = revision.value
        loading.value = true
        error.value = null
        try {
          const result = await read(a, b, columns)
          if (!disposed && version === revision.value) {
            dataRevision = version
            data.value = result
          }
        }
        catch (e) {
          if (!disposed && version === revision.value)
            error.value = String(e)
        }
      }
    }
    finally {
      reading = false
      loading.value = false
    }
  }

  watch([enabled, revision, frames, start, end, width], refresh, { immediate: true })
  onScopeDispose(() => { disposed = true })
  return { data, loading, error, refresh }
}
