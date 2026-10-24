import { onScopeDispose } from 'vue'

/** Coalesce canvas invalidations into one draw per animation frame. */
export function useFrameDraw(draw: () => void) {
  let pending: number | undefined
  let disposed = false

  onScopeDispose(() => {
    disposed = true
    if (pending !== undefined)
      cancelAnimationFrame(pending)
  })

  return () => {
    if (disposed || pending !== undefined)
      return
    pending = requestAnimationFrame(() => {
      pending = undefined
      draw()
    })
  }
}
