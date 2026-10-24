import type { Ref } from 'vue'
import { clamp } from './color'

export interface TrackPosition {
  /** Normalised 0..1 from the left edge. */
  x: number
  /** Normalised 0..1 from the top edge. */
  y: number
}

/**
 * Pointer-drag helper shared by the 2D selection area and the 1D sliders.
 *
 * Reports the cursor position as `{ x, y }` normalised to 0..1 within the bound
 * element. Pointer capture keeps the drag alive while the cursor leaves the
 * element, so the handle stays glued to the pointer until release.
 */
export function usePointerTrack(
  elRef: Ref<HTMLElement | null>,
  onMove: (pos: TrackPosition) => void,
) {
  let dragging = false

  function report(event: PointerEvent) {
    const el = elRef.value
    if (!el)
      return
    const rect = el.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0)
      return
    onMove({
      x: clamp((event.clientX - rect.left) / rect.width, 0, 1),
      y: clamp((event.clientY - rect.top) / rect.height, 0, 1),
    })
  }

  function onPointerDown(event: PointerEvent) {
    // Ignore secondary buttons so right-click / context menus pass through.
    if (event.button !== 0)
      return
    dragging = true
    ;(event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId)
    report(event)
    event.preventDefault()
  }

  function onPointerMove(event: PointerEvent) {
    if (dragging)
      report(event)
  }

  function onPointerUp(event: PointerEvent) {
    if (!dragging)
      return
    dragging = false
    ;(event.currentTarget as HTMLElement).releasePointerCapture?.(event.pointerId)
  }

  return { onPointerDown, onPointerMove, onPointerUp }
}
