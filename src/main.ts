import { getCurrentWindow } from '@tauri-apps/api/window'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import { i18n } from './i18n'
import 'vue-sonner/style.css'
import 'virtual:uno.css'
import './styles.css'

// Editor windows load the same page with ?editor=<sample id>.
const editorId = Number(new URLSearchParams(location.search).get('editor'))
async function mount() {
  // Select the window before loading its component tree. An editor window does
  // not need the library shell; the main window loads the editor when requested.
  // Keep the await inside each branch: Vite's preload transform can otherwise
  // hoist the library's dependency preload outside the conditional import.
  const root = editorId
    ? createApp((await import('./EditorWindow.vue')).default, { sampleId: editorId })
    : createApp((await import('./App.vue')).default)
  root.use(createPinia()).use(i18n).mount('#app')

  // Windows start hidden (tauri.conf, editor builder) and appear only now that the page is
  // styled and mounted, instead of flashing a white webview first. Outside Tauri there is
  // no window to show.
  if ('__TAURI_INTERNALS__' in window)
    getCurrentWindow().show().catch(err => console.error('show window failed', err))
}
void mount().catch(err => console.error('mount window failed', err))
