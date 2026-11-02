import type { Ref } from 'vue'
import type { Wave } from '../../stores/editor.ts'
import { onScopeDispose, shallowRef, watch } from 'vue'

interface Options {
  enabled: Ref<boolean>
  identity: Ref<string>
  revision: Ref<number>
  frames: Ref<number>
  start: Ref<number>
  end: Ref<number>
  width: Ref<number>
  pixelRatio: Ref<number>
  split: Ref<boolean>
  bands: Ref<boolean>
  read: (start: number, end: number, columns: number, split: boolean, bands: boolean) => Promise<Wave>
}

/** One viewport request at a time, with overscan and stale-session protection. */
export function useEditorWaveform(options: Options) {
  const data = shallowRef<Wave | null>(null)
  const error = shallowRef<string | null>(null)
  const range = shallowRef({ start: 0, end: 0, key: '' })
  let reading = false, again = false, disposed = false
  const key = () => `${options.identity.value}|${options.revision.value}|${options.split.value}|${options.bands.value}`
  function request() {
    const span = options.end.value - options.start.value
    const n = options.frames.value
    if (options.width.value <= 0 || n <= 0 || span <= 0) return null
    const wide = Math.min(n, span * 1.5)
    const start = Math.floor(Math.max(0, Math.min(n - wide, options.start.value - (wide - span) / 2)))
    const end = Math.ceil(Math.min(n, start + wide))
    const columns = Math.min(16384, Math.max(1, Math.round(options.width.value * options.pixelRatio.value * (end - start) / span)))
    return { start, end, columns }
  }
  function fits(target: NonNullable<ReturnType<typeof request>>) {
    const wave = data.value, r = range.value
    if (!wave || r.key !== key() || r.start > options.start.value || r.end < options.end.value) return false
    // Compare against the density a new capped request can actually supply.
    // Use the new viewport's span so a zoom still refines an older coarse cache.
    const framesPerColumn = (target.end - target.start) / target.columns
    if (wave.samples) return framesPerColumn <= 1
    const ratio = ((r.end - r.start) / wave.count) / framesPerColumn
    return ratio > 0.9 && ratio < 1.1
  }
  async function refresh() {
    again = true
    if (reading || disposed) return
    reading = true
    try {
      while (again && !disposed) {
        again = false
        if (!options.enabled.value) continue
        const target = request()
        if (!target || fits(target)) continue
        const { start, end, columns } = target
        const version = key()
        error.value = null
        try {
          const next = await options.read(start, end, columns, options.split.value, options.bands.value)
          if (disposed) return
          if (version !== key() || !options.enabled.value) { again = true; continue }
          range.value = { start, end, key: version }
          data.value = next
        }
        catch (e) {
          if (!disposed && version === key()) error.value = String(e)
        }
      }
    }
    finally { reading = false }
  }
  watch(options.identity, () => { data.value = null; error.value = null }, { flush: 'sync' })
  watch([options.enabled, options.identity, options.revision, options.frames, options.start,
    options.end, options.width, options.pixelRatio, options.split, options.bands], refresh, { immediate: true })
  onScopeDispose(() => { disposed = true })
  return { data, range, error, refresh }
}
