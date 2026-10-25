<script setup lang="ts">
import type { ModelStorage } from '@/api'
import { Download, FolderOpen, Trash2 } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { storeToRefs } from 'pinia'
import { computed, onMounted, onUnmounted, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { confirm } from '@/composables/confirm'
import { formatSize } from '@/format'
import { useLibraryStore } from '@/stores/library'

const emit = defineEmits<{ changed: [] }>()
const { t } = useI18n()
const library = useLibraryStore()
const { modelDownload } = storeToRefs(library)
const models = shallowRef<ModelStorage[] | null>(null)
const error = shallowRef('')
const busy = shallowRef(false)
const total = computed(() => models.value?.reduce((sum, model) => sum + model.size, 0) ?? 0)
let active = true
let timer: ReturnType<typeof setTimeout> | undefined

async function refresh() {
  clearTimeout(timer)
  try {
    const next = await api.modelStorage()
    if (active) {
      models.value = next
      error.value = ''
    }
  }
  catch (e) {
    if (active) error.value = String(e)
  }
  finally {
    // This section is mounted only while visible. Also picks up downloads from
    // another window and files removed outside the app, without a new watcher.
    if (active) timer = setTimeout(refresh, 2000)
  }
}
onMounted(refresh)
onUnmounted(() => { active = false; clearTimeout(timer) })

function status(model: ModelStorage) {
  if (model.downloading)
    return t('settings.models.downloading', { size: formatSize(model.size), total: formatSize(model.downloadSize) })
  if (model.installed)
    return t('settings.models.installed', { size: formatSize(model.size) })
  if (model.size > 0)
    return t('settings.models.incomplete', { size: formatSize(model.size), total: formatSize(model.downloadSize) })
  return t('settings.models.missing', { size: formatSize(model.downloadSize) })
}

async function reveal(path: string) {
  try { await revealItemInDir(path) }
  catch (e) { toast.error(String(e)) }
}

async function download() {
  busy.value = true
  try { await library.downloadSoundModel(); await refresh() }
  catch (e) { toast.error(String(e)) }
  finally { busy.value = false }
}

async function remove() {
  const ok = await confirm({
    title: t('settings.analysis.deleteTitle'),
    description: t('settings.analysis.deleteDesc'),
    action: t('common.actions.delete'),
    destructive: true,
  })
  if (!ok) return
  busy.value = true
  try {
    await library.deleteSoundModel()
    await refresh()
    emit('changed')
  }
  catch (e) { toast.error(String(e)) }
  finally { busy.value = false }
}
</script>

<template>
  <section class="flex flex-col gap-2" :aria-label="t('settings.models.title')">
    <div class="flex items-center justify-between gap-4 text-xs text-muted-foreground">
      <h3 class="font-medium">{{ t('settings.models.title') }}</h3>
      <span v-if="models" class="tabular">{{ t('settings.models.total', { size: formatSize(total) }) }}</span>
    </div>
    <p v-if="error" role="alert" class="text-xs text-destructive">{{ error }}</p>
    <p v-if="!models && !error" role="status" class="text-xs text-muted-foreground">{{ t('settings.models.loading') }}</p>
    <div v-if="models" class="rounded-[var(--radius)] border border-border bg-card">
      <div v-for="(model, index) in models" :key="model.id" class="flex items-center gap-4 px-4 py-3" :class="{ 'border-t border-border': index > 0 }">
        <div class="flex-1 min-w-0">
          <div class="font-medium">{{ t(`settings.models.${model.id}.name`) }}</div>
          <div class="mt-0.5 text-xs text-muted-foreground">{{ t(`settings.models.${model.id}.description`) }}</div>
          <div class="mt-1 text-xs text-muted-foreground tabular" role="status">{{ status(model) }}</div>
          <p v-if="model.size > 0" class="mt-1 break-all select-text text-xs text-muted-foreground" :title="model.path">{{ t('settings.models.location', { path: model.path }) }}</p>
          <div v-if="model.downloading" class="mt-2 h-1 rounded-full bg-muted overflow-hidden" role="progressbar" :aria-label="t(`settings.models.${model.id}.name`)" :aria-valuenow="Math.min(100, Math.round(model.size / model.downloadSize * 100))" aria-valuemin="0" aria-valuemax="100">
            <div class="h-full bg-primary transition-[width]" :style="{ width: `${Math.min(100, model.size / model.downloadSize * 100)}%` }" />
          </div>
          <p v-if="model.id === 'clap' && modelDownload?.error" role="alert" class="mt-1 text-xs text-destructive">{{ modelDownload.error }}</p>
          <p v-if="model.id !== 'clap' && model.id !== 'apollo' && !model.installed && !model.downloading" class="mt-1 text-xs text-muted-foreground">{{ t('settings.models.onSeparation') }}</p>
        </div>
        <div class="flex gap-0.5">
          <Hint v-if="model.installed || (model.id !== 'clap' && model.size > 0)" :text="t('common.actions.showInFinder')">
            <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="`${t('common.actions.showInFinder')}: ${t(`settings.models.${model.id}.name`)}`" @click="reveal(model.path)">
              <FolderOpen />
            </Button>
          </Hint>
          <template v-if="model.id === 'clap'">
            <Hint v-if="model.installed" :text="t('settings.analysis.deleteModel')">
              <Button variant="ghost" size="icon-sm" class="text-muted-foreground hover:text-destructive" :disabled="busy || model.downloading" :aria-label="t('settings.analysis.deleteModel')" @click="remove">
                <Trash2 />
              </Button>
            </Hint>
            <Button v-else variant="outline" size="sm" :disabled="busy || model.downloading" @click="download">
              <Download /> {{ t('settings.analysis.download') }}
            </Button>
          </template>
        </div>
      </div>
    </div>
  </section>
</template>
