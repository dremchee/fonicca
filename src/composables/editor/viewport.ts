import type { Ref } from 'vue'
import type { Span } from '@/stores/editor'
import { shallowRef, watch } from 'vue'

/** Visible frame range and bounded navigation, independent of the canvas. */
export function useEditorViewport(frames: Ref<number>, selection: () => Span | null) {
  const viewStart = shallowRef(0)
  const viewEnd = shallowRef(1)

  function fit() {
    viewStart.value = 0
    viewEnd.value = Math.max(1, frames.value)
  }

  // After an edit shortens the audio, keep the view inside it.
  watch(frames, (n) => {
    if (viewEnd.value > n || viewEnd.value - viewStart.value <= 0) {
      const span = Math.min(viewEnd.value - viewStart.value, n)
      viewEnd.value = Math.max(1, n)
      viewStart.value = Math.max(0, n - span)
    }
  })

  function zoom(factor: number, around = (viewStart.value + viewEnd.value) / 2) {
    const span = Math.min(frames.value, Math.max(64, (viewEnd.value - viewStart.value) * factor))
    const ratio = (around - viewStart.value) / (viewEnd.value - viewStart.value)
    let start = around - span * ratio
    start = Math.max(0, Math.min(frames.value - span, start))
    viewStart.value = start
    viewEnd.value = start + span
  }

  function pan(deltaFrames: number) {
    const span = viewEnd.value - viewStart.value
    const start = Math.max(0, Math.min(frames.value - span, viewStart.value + deltaFrames))
    viewStart.value = start
    viewEnd.value = start + span
  }

  function zoomToSelection() {
    const s = selection()
    if (s) {
      viewStart.value = s.start
      viewEnd.value = Math.max(s.end, s.start + 64)
    }
  }

  return { viewStart, viewEnd, fit, zoom, pan, zoomToSelection }
}
