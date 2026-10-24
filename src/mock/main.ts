// Clickable design mocks for a plain browser, not part of the app build (only
// `pnpm dev` serves mock.html): /mock.html is the real app on a fake backend,
// /mock.html#editor an editor window on it, /mock.html#export the kit export design,
// /mock.html#themes the theme editor design.
import { mockWindows } from '@tauri-apps/api/mocks'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from '../App.vue'
import EditorWindow from '../EditorWindow.vue'
import ProjectWindow from '../ProjectWindow.vue'
import { i18n } from '../i18n'
import ExportMock from './ExportMock.vue'
import ThemesMock from './ThemesMock.vue'
import { install } from './real-app'
import { openBatch } from '@/composables/batch'
import 'vue-sonner/style.css'
import 'virtual:uno.css'
import '../styles.css'

if (location.hash === '#export') {
  createApp(ExportMock).use(i18n).mount('#app')
}
else if (location.hash === '#themes') {
  createApp(ThemesMock).use(i18n).mount('#app')
}
else if (location.hash === '#editor') {
  mockWindows('editor-1')
  install()
  createApp(EditorWindow, { sampleId: 1 }).use(createPinia()).use(i18n).mount('#app')
}
else if (location.hash === '#project') {
  mockWindows('project')
  install()
  createApp(ProjectWindow).use(createPinia()).use(i18n).mount('#app')
}
else {
  mockWindows('main')
  install()
  createApp(App).use(createPinia()).use(i18n).mount('#app')
  if (location.hash === '#batch')
    openBatch([1, 2, 3])
}
