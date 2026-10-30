<script setup lang="ts">
import { defineAsyncComponent, ref, watch } from 'vue'
import { useSettingsStore } from '@/stores/settings'

withDefaults(defineProps<{ embedded?: boolean, waveTarget?: 'preview' | 'editor' }>(), { embedded: false, waveTarget: 'preview' })
const SettingsDialog = defineAsyncComponent(() => import('./SettingsDialog.vue'))
const settings = useSettingsStore()
const activated = ref(false)
// Load once on first open, then retain the same component while hidden, as
// before. Also handles opening settings before the window has finished mounting.
watch(() => settings.dialogOpen, open => { if (open) activated.value = true }, { immediate: true })
</script>

<template>
  <template v-if="activated">
    <SettingsDialog v-if="embedded" embedded v-show="settings.dialogOpen" :wave-target="waveTarget" />
    <SettingsDialog v-else :wave-target="waveTarget" />
  </template>
</template>
