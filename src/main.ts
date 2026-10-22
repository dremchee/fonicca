import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from './App.vue'
import 'virtual:uno.css'
import './styles.css'

createApp(App).use(createPinia()).mount('#app')
