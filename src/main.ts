import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from './App.vue'
import 'vue-sonner/style.css'
import 'virtual:uno.css'
import './styles.css'

createApp(App).use(createPinia()).mount('#app')
