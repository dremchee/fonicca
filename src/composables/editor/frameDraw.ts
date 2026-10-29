import { onScopeDispose } from 'vue'
import { editorTimings } from '../../editor-performance.ts'

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
      editorTimings.measureSync('editor.draw', draw)
    })
  }
}
