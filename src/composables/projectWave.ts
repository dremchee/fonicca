import { invoke } from '@tauri-apps/api/core'
export interface ProjectWave { duration: number, peaks: number[] }
// Decode overviews serially, shared by repeated clips. Keep the UI cache bounded.
const cache = new Map<string, Promise<ProjectWave>>()
let queue: Promise<unknown> = Promise.resolve()
export function projectWave(path: string) {
  let request = cache.get(path)
  if (!request) {
    request = queue.catch(() => {}).then(() => invoke<ProjectWave>('project_wave', { path }))
    queue = request
    cache.set(path, request)
    if (cache.size > 32) cache.delete(cache.keys().next().value!)
    void request.catch(() => cache.delete(path))
  }
  return request
}
