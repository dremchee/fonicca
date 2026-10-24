import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { onBeforeUnmount, onMounted } from 'vue'
import { useProjectStore } from '@/stores/project'

/** Register before draining: requests queued during page startup cannot get lost. */
export function useProjectWindow() {
  const project = useProjectStore()
  let disposed = false
  let draining = false
  const unlisten: UnlistenFn[] = []
  async function drain() {
    if (draining || disposed) return
    draining = true
    try {
      while (!disposed) {
        const requests = await invoke<number[][]>('project_take_samples')
        if (!requests.length) break
        for (const ids of requests) { if (!disposed) await project.addSamples(ids) }
      }
    }
    catch (error) { if (!disposed) project.error = String(error) }
    finally { draining = false }
  }
  async function keep(subscription: Promise<UnlistenFn>) {
    const stop = await subscription
    if (disposed) stop(); else unlisten.push(stop)
  }
  onMounted(async () => {
    try {
      await keep(listen('project:samples-ready', drain))
      await drain()
      await keep(getCurrentWindow().onDragDropEvent(({ payload }) => {
        if (payload.type !== 'drop') return
        const x = payload.position.x / window.devicePixelRatio, y = payload.position.y / window.devicePixelRatio
        const lane = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-project-track]')
        window.dispatchEvent(new CustomEvent('project:file-drop', { detail: { paths: payload.paths, x, y, trackId: lane?.dataset.projectTrack } }))
      }))
    }
    catch (error) { if (!disposed) project.error = String(error) }
  })
  onBeforeUnmount(() => { disposed = true; unlisten.forEach(stop => stop()); project.stop() })
}
