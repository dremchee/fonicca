import type { Settings } from '@/api'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { ref } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api'

export type SettingsSection = 'general' | 'library' | 'playback' | 'analysis' | 'shortcuts' | 'about'

// Mirrors settings.json, which the backend owns; used until it loads.
const DEFAULTS: Settings = { theme: 'system', watchFolders: true, autoplay: true, volume: 1, analysisThreads: null }

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings>({ ...DEFAULTS })
  const dialogOpen = ref(false)
  const section = ref<SettingsSection>('general')

  async function init() {
    settings.value = await api.getSettings()
  }

  /** Applies right away; the backend stores the full object and clamps values. */
  async function update(patch: Partial<Settings>) {
    const prev = settings.value
    settings.value = { ...prev, ...patch }
    try {
      settings.value = await api.updateSettings(settings.value)
    }
    catch (e) {
      settings.value = prev
      toast.error(String(e))
    }
  }

  function openDialog(at?: SettingsSection) {
    if (at)
      section.value = at
    dialogOpen.value = true
  }

  return { settings, dialogOpen, section, init, update, openDialog }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useSettingsStore, import.meta.hot))
