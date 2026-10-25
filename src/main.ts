import { getCurrentWindow } from '@tauri-apps/api/window'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from './App.vue'
import EditorWindow from './EditorWindow.vue'
import { i18n } from './i18n'
import 'vue-sonner/style.css'
import 'virtual:uno.css'
import './styles.css'

// Editor windows load the same page with ?editor=<sample id>.
const editorId = Number(new URLSearchParams(location.search).get('editor'))
const root = editorId ? createApp(EditorWindow, { sampleId: editorId }) : createApp(App)
root.use(createPinia()).use(i18n).mount('#app')

// Windows start hidden (tauri.conf, editor builder) and appear only now that the page is
// styled and mounted, instead of flashing a white webview first. Outside Tauri there is
// no window to show.
if ('__TAURI_INTERNALS__' in window)
  getCurrentWindow().show().catch(err => console.error('show window failed', err))
