import { useDocumentVisibility, useStorage } from '@vueuse/core'
import { computed, onScopeDispose, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { editorTimings, type TimingSummary } from '@/editor-performance'

export interface EditorDiagnostics {
  sampleId: number
  logicalBytes: number
  ramBytes: number
  diskBytes: number
  storageCount: number
  cacheBytes: number
  undoSteps: number
  redoSteps: number
  openedMs: number
  lastWaveMs: number | null
}
export interface DebugSnapshot {
  demo?: boolean
  processBytes: number | null
  cpuPercent: number | null
  availableBytes: number
  editorBusy: boolean
  editor: EditorDiagnostics | null
}

// Shared by the footer and overlay; deliberately independent of audio settings.
export const debugMonitorOpen = useStorage('fonicca.debug-monitor', false)

export function useDebugSampler() {
  const snapshot = shallowRef<DebugSnapshot | null>(null)
  const history = shallowRef<number[]>([])
  const timings = shallowRef<TimingSummary[]>([])
  const heapBytes = shallowRef<number | null>(null)
  const error = shallowRef(false)
  const paused = shallowRef(false)
  const baseline = shallowRef<number | null>(null)
  const peak = shallowRef<number | null>(null)
  const visibility = useDocumentVisibility()
  let inFlight = false
  let generation = 0
  let timer: ReturnType<typeof setTimeout> | undefined
  const active = computed(() => debugMonitorOpen.value && !paused.value && visibility.value === 'visible')

  function reset() {
    baseline.value = snapshot.value?.processBytes ?? null
    peak.value = baseline.value
    history.value = []
    editorTimings.reset()
    timings.value = []
  }
  async function poll(version: number) {
    if (!active.value || version !== generation) return
    if (!inFlight) {
      inFlight = true
      try {
        const next = await invoke<DebugSnapshot>('debug_snapshot')
        if (version !== generation) return
        snapshot.value = next
        timings.value = editorTimings.snapshot()
        // Chromium-only JS heap estimate, not total WebView process RSS.
        const memory = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory
        heapBytes.value = memory?.usedJSHeapSize ?? null
        error.value = false
        if (next.processBytes !== null) {
          baseline.value ??= next.processBytes
          peak.value = Math.max(peak.value ?? 0, next.processBytes)
          history.value = [...history.value.slice(-59), next.processBytes]
        }
      }
      catch { if (version === generation) error.value = true }
      finally { inFlight = false }
    }
    if (active.value && version === generation) timer = setTimeout(() => void poll(version), 1000)
  }
  watch(active, () => {
    clearTimeout(timer)
    const version = ++generation
    if (active.value) void poll(version)
  }, { immediate: true })
  onScopeDispose(() => { generation++; clearTimeout(timer) })
  return { snapshot, history, timings, heapBytes, error, paused, baseline, peak, reset }
}

export function debugBytes(bytes: number | null | undefined): string {
  if (bytes == null) return '—'
  const abs = Math.abs(bytes)
  const unit = abs >= 1024 ** 3 ? 'GiB' : 'MiB'
  return `${(bytes / (unit === 'GiB' ? 1024 ** 3 : 1024 ** 2)).toFixed(1)} ${unit}`
}
