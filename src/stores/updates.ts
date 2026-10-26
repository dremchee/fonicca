import { invoke, isTauri } from '@tauri-apps/api/core'
import { check as checkUpdate } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { defineStore } from 'pinia'
import { computed, shallowRef } from 'vue'
import { toast } from 'vue-sonner'
import { confirm } from '@/composables/confirm'
import { t } from '@/i18n'
import { createUpdateController, type UpdateSnapshot } from '@/update-controller'
import { useSettingsStore } from '@/stores/settings'
import { usePlayerStore } from '@/stores/player'

interface NativeStatus { configured: boolean, supported: boolean, development: boolean, editorsOpen: boolean, busy: boolean }

export const useUpdatesStore = defineStore('updates', () => {
  const state = shallowRef<UpdateSnapshot>({ phase: 'idle', version: '', notes: '', received: 0, total: 0, error: '' })
  const availability = shallowRef<'loading' | 'enabled' | 'development' | 'package' | 'browser' | 'unconfigured'>('loading')
  const prompting = shallowRef(false)
  const busy = computed(() => prompting.value || ['checking', 'downloading', 'installing'].includes(state.value.phase))
  const percent = computed(() => state.value.total > 0 ? Math.min(100, Math.round(100 * state.value.received / state.value.total)) : null)
  let initialized = false
  const controller = createUpdateController({
    check: () => checkUpdate({ timeout: 30_000 }),
    beforeInstall: async () => {
      const status = await invoke<NativeStatus>('update_status')
      if (status.editorsOpen) throw t('updates.closeEditors')
      if (status.busy) throw t('updates.finishJobs')
      await usePlayerStore().stop()
    },
    relaunch,
  }, snapshot => { state.value = snapshot })

  async function check(automatic = false) {
    if (availability.value !== 'enabled' || busy.value) return
    await controller.check()
    if (automatic && state.value.phase === 'available') {
      toast(t('updates.available', { version: state.value.version }), {
        duration: 12_000,
        action: { label: t('updates.details'), onClick: () => useSettingsStore().openDialog('about') },
      })
    }
    // Startup failures stay in About; a temporarily offline app should open quietly.
  }
  async function init() {
    if (initialized) return
    initialized = true
    if (!isTauri()) { availability.value = 'browser'; return }
    try {
      const status = await invoke<NativeStatus>('update_status')
      availability.value = status.development ? 'development' : !status.configured ? 'unconfigured' : status.supported ? 'enabled' : 'package'
      await check(true)
    }
    catch (error) {
      availability.value = 'browser'
      console.error('Updater initialization failed:', error)
    }
  }
  async function install() {
    if (availability.value !== 'enabled' || busy.value || !['ready', 'installed'].includes(state.value.phase)) return
    prompting.value = true
    try {
      if (await confirm({ title: t('updates.installTitle'), description: t('updates.installDescription'), action: t('updates.install') }))
        await controller.install()
    }
    finally { prompting.value = false }
  }
  function download() {
    if (availability.value === 'enabled' && !busy.value) return controller.download()
  }
  return { state, availability, busy, percent, init, check, download, install }
})
