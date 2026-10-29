<script setup lang="ts">
import type { ModelStorage } from '@/api'
import { Download } from '@lucide/vue'
import { computed, onMounted, onUnmounted, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import { Badge } from '@/components/ui/badge'
import SoundModelDownload from '@/components/SoundModelDownload.vue'
import { useLibraryStore } from '@/stores/library'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const library = useLibraryStore()
const settings = useSettingsStore()
const models = shallowRef<ModelStorage[]>([])
let active = true
let timer: ReturnType<typeof setTimeout> | undefined
const downloads = computed(() => {
  const items = models.value.filter(model => model.downloading).map(model => ({
    id: model.id,
    percent: model.downloadSize ? Math.min(100, Math.max(0, Math.floor(model.size / model.downloadSize * 100))) : 0,
  }))
  const clap = library.modelDownload
  if (clap && !clap.done && !clap.error) {
    const item = { id: 'clap' as const, percent: clap.total ? Math.min(100, Math.max(0, Math.floor(clap.received / clap.total * 100))) : 0 }
    const index = items.findIndex(model => model.id === 'clap')
    if (index === -1) items.unshift(item)
    else items[index] = item
  }
  return items
})
const showSoundModel = computed(() => !downloads.value.some(model => model.id === 'clap') && (
  !!library.modelDownload?.error || !!library.soundModel && !library.soundModel.installed
))
async function refresh() {
  try {
    const result = await api.modelStorage()
    if (active) models.value = result
  }
  catch { /* Keep the last state during a transient connection failure. */ }
  finally { if (active) timer = setTimeout(refresh, 1000) }
}
onMounted(refresh)
onUnmounted(() => { active = false; clearTimeout(timer) })
</script>

<template>
  <div v-if="downloads.length || showSoundModel" class="flex flex-col gap-1 py-2">
    <Badge v-for="model in downloads" :key="model.id" as-child variant="secondary" class="download-badge">
      <button type="button" :aria-label="t('filters.modelDownload', { name: t(`settings.models.${model.id}.name`), percent: model.percent })" @click="settings.openDialog('analysis')">
        <span class="download-fill" aria-hidden="true" :style="{ transform: `scaleX(${model.percent / 100})` }" />
        <Download class="relative size-3 shrink-0" aria-hidden="true" />
        <span class="relative min-w-0 flex-1 truncate text-left">{{ t(`settings.models.${model.id}.name`) }}</span>
        <span class="relative shrink-0 tabular" role="progressbar" :aria-label="t(`settings.models.${model.id}.name`)" :aria-valuenow="model.percent" aria-valuemin="0" aria-valuemax="100">{{ model.percent }}%</span>
      </button>
    </Badge>
    <SoundModelDownload v-if="showSoundModel" />
  </div>
</template>

<style scoped>
.download-badge { position: relative; display: inline-flex; width: 100%; max-width: 100%; gap: 6px; overflow: hidden; border-radius: 999px; padding: 5px 10px; font-size: 11px; line-height: 16px; font-weight: 500; color: var(--foreground); cursor: pointer; }
.download-fill { position: absolute; inset: 0; transform-origin: left; background: color-mix(in srgb, var(--primary) 18%, transparent); transition: transform 200ms linear; }
.download-badge:hover { background: var(--accent); }
.download-badge:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) { .download-fill { transition: none; } }
</style>
