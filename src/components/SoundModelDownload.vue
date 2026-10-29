<script setup lang="ts">
import { Download, RotateCcw } from '@lucide/vue'
import { computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Spinner } from '@/components/ui/spinner'
import Hint from '@/components/Hint.vue'
import { formatSize } from '@/format'
import { useLibraryStore } from '@/stores/library'

const { t } = useI18n()
const library = useLibraryStore()
const { soundModel, modelDownload } = storeToRefs(library)
const downloading = computed(() => !!modelDownload.value && !modelDownload.value.error)
const percent = computed(() => {
  const d = modelDownload.value
  return d?.total ? Math.round(d.received / d.total * 100) : 0
})
async function download() {
  try { await library.downloadSoundModel() }
  catch (error) { toast.error(String(error)) }
}
</script>

<template>
  <Hint v-if="modelDownload || (soundModel && !soundModel.installed)" :text="modelDownload?.error || t('filters.soundModelHint')">
    <button type="button" class="model-row" :disabled="downloading" @click="download">
      <Spinner v-if="downloading" class="size-4 shrink-0" />
      <RotateCcw v-else-if="modelDownload?.error" class="size-4 shrink-0 text-destructive" />
      <Download v-else class="size-4 shrink-0" />
      <span class="min-w-0 truncate" :class="{ 'text-destructive': modelDownload?.error }">
        {{ modelDownload?.error ? t('filters.downloadFailed') : downloading ? t('filters.downloadingModel', { percent }) : t('filters.soundModel', { size: formatSize(soundModel?.size ?? 0) }) }}
      </span>
    </button>
  </Hint>
</template>

<style scoped>
.model-row { display: flex; align-items: center; gap: 8px; width: 100%; border-radius: var(--radius); padding: 6px 8px; font-size: 13px; color: var(--muted-foreground); text-align: left; }
.model-row:hover:not(:disabled) { background: var(--accent); color: var(--foreground); }
.model-row:focus-visible { outline: 2px solid var(--ring); outline-offset: -2px; }
.model-row:disabled { cursor: default; }
</style>
