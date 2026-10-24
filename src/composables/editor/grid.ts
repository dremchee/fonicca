import type { Ref } from 'vue'
import { computed } from 'vue'

export type GridMode = 'beats' | 'time' | 'off'
export const MIN_LINE_PX = 12

interface GridOptions {
  bpm: Ref<number | null>
  rate: Ref<number>
  frames: Ref<number>
  width: Ref<number>
  viewStart: Ref<number>
  viewEnd: Ref<number>
  downbeat: Ref<number>
  gridMode: Ref<GridMode>
  snap: Ref<boolean>
}

/** Beat/time grid shared by drawing, selection and marker snapping. */
export function useEditorGrid(options: GridOptions) {
  const { bpm, rate, frames, width, viewStart, viewEnd, downbeat, gridMode, snap } = options
  const beatFrames = computed(() => (bpm.value && bpm.value > 0 ? (60 / bpm.value) * rate.value : 0))
  // Bars and beats when the tempo is known, otherwise seconds. 4/4 assumed.
  const beatGrid = computed(() => gridMode.value === 'beats' && beatFrames.value > 0)
  const BEAT_STEPS = [1 / 8, 1 / 4, 1 / 2, 1, 4, 8, 16, 32, 64, 128, 256]
  const TIME_STEPS = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600]

  /** The finest grid step, in frames, that is at least `minPx` wide on screen. */
  function gridStep(minPx: number) {
    const pxPerFrame = width.value / (viewEnd.value - viewStart.value)
    const unit = beatGrid.value ? beatFrames.value : rate.value
    const steps = beatGrid.value ? BEAT_STEPS : TIME_STEPS
    return (steps.find(s => s * unit * pxPerFrame >= minPx) ?? steps.at(-1)!) * unit
  }

  const gridOrigin = () => (beatGrid.value ? downbeat.value : 0)

  /** Visible multiples of `step` from the grid origin. */
  function gridFrames(step: number) {
    const origin = gridOrigin()
    const out: number[] = []
    for (let k = Math.ceil((viewStart.value - origin) / step); origin + k * step <= viewEnd.value; k++)
      out.push(origin + k * step)
    return out
  }

  const isMultiple = (value: number, of: number) => Math.abs(value / of - Math.round(value / of)) < 1e-6

  /** Snaps a frame to the nearest grid line (and to the end of the file) while snapping is on; ⌥ inverts it. */
  function snapped(frame: number, e: MouseEvent) {
    if (gridMode.value === 'off' || snap.value === e.altKey)
      return frame
    const step = gridStep(MIN_LINE_PX)
    const origin = gridOrigin()
    const line = origin + Math.round((frame - origin) / step) * step
    const nearEnd = Math.abs(frames.value - frame) < Math.abs(line - frame)
    return Math.round(Math.min(frames.value, Math.max(0, nearEnd ? frames.value : line)))
  }

  return { beatFrames, beatGrid, gridStep, gridOrigin, gridFrames, isMultiple, snapped }
}
