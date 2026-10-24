// Clickable design mocks for a plain browser, not part of the app build (only
// `pnpm dev` serves mock.html): /mock.html is the real app on a fake backend,
// /mock.html#export the kit export design.
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from '../App.vue'
import { i18n } from '../i18n'
import ExportMock from './ExportMock.vue'
import { install } from './real-app'
import 'vue-sonner/style.css'
import 'virtual:uno.css'
import '../styles.css'

if (location.hash === '#export') {
  createApp(ExportMock).use(i18n).mount('#app')
}
else {
  install()
  createApp(App).use(createPinia()).use(i18n).mount('#app')
}
