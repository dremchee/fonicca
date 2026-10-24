import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'

/** Only the project window owns its draft. The library sends requests through Rust. */
export async function openProjectWindow(ids: number[] = []) {
  try { await invoke('open_project_window', { ids }) }
  catch (error) { toast.error(String(error)) }
}
