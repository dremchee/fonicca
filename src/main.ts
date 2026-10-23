import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from './App.vue'
import EditorWindow from './EditorWindow.vue'
import 'vue-sonner/style.css'
import 'virtual:uno.css'
import './styles.css'

// Editor windows load the same page with ?editor=<sample id>.
const editorId = Number(new URLSearchParams(location.search).get('editor'))
const root = editorId ? createApp(EditorWindow, { sampleId: editorId }) : createApp(App)
root.use(createPinia()).mount('#app')
