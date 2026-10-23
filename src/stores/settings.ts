import type { Settings } from '@/api'
import { listen } from '@tauri-apps/api/event'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { setLocale } from '@/i18n'

export type SettingsSection = 'general' | 'library' | 'playback' | 'waveform' | 'analysis' | 'shortcuts' | 'about'

// Mirrors settings.json, which the backend owns; used until it loads.
const DEFAULTS: Settings = {
  theme: 'system',
  language: 'en',
  watchFolders: true,
  autoplay: true,
  volume: 1,
  analysisThreads: null,
  syncEnabled: false,
  projectBpm: null,
  projectKey: null,
  waveStyle: 'peaksRms',
  waveChannels: 'split',
  waveScale: 'linear',
  waveColors: 'theme',
}

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings>({ ...DEFAULTS })
  const dialogOpen = ref(false)
  const section = ref<SettingsSection>('general')

  // Every window follows the stored language, including changes made in another window.
  watch(() => settings.value.language, setLocale, { immediate: true })

  async function init() {
    settings.value = await api.getSettings()
    // Changes made in another window (e.g. the main one while an editor is open).
    await listen<Settings>('settings-changed', ({ payload }) => {
      settings.value = payload
    })
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
