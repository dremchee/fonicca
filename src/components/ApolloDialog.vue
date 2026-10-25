<script setup lang="ts">
import { FolderOpen, Pencil, Play } from '@lucide/vue'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { formatSize } from '@/format'
import { useApolloStore } from '@/stores/apollo'
import { usePlayerStore } from '@/stores/player'
const apollo = useApolloStore()
const player = usePlayerStore()
const { t } = useI18n()
const result = computed(() => {
  const p = apollo.progress
  if (p.running || !p.path || p.id == null || !apollo.sample || p.sourceId !== apollo.sample.id) return null
  return { ...apollo.sample, id: p.id, path: p.path, name: p.path.split(/[/\\]/).pop()!.replace(/\.wav$/i, ''), ext: 'wav', sampleRate: 44100, bitDepth: 32 }
})
async function edit() {
  if (!result.value) return
  try { await invoke('open_editor_window', { id: result.value.id }) }
  catch (e) { apollo.error = String(e) }
}
</script>
<template>
  <Dialog v-model:open="apollo.open">
    <DialogContent class="sm:max-w-lg max-h-[90vh] overflow-y-auto">
      <DialogHeader><DialogTitle>{{ t('apollo.title') }}</DialogTitle><DialogDescription>{{ t('apollo.description') }}</DialogDescription></DialogHeader>
      <p class="truncate text-sm font-medium">{{ apollo.progress.running ? apollo.progress.name : apollo.sample?.name }}</p>
      <p class="text-xs text-muted-foreground">{{ apollo.ready ? t('apollo.ready') : t('apollo.download', { size: formatSize(apollo.bytes) }) }}</p>
      <p class="text-xs text-muted-foreground">{{ t('apollo.output') }}</p>
      <p class="text-xs text-muted-foreground">{{ t('apollo.limits') }}</p>
      <div v-if="apollo.progress.stage && (apollo.progress.running || apollo.progress.sourceId === apollo.sample?.id)" class="flex flex-col gap-2" aria-live="polite">
        <div class="flex justify-between text-xs"><span>{{ apollo.cancelling ? t('apollo.stopping') : t(`apollo.stages.${apollo.progress.stage}`) }}</span><span v-if="apollo.progress.running">{{ Math.round(apollo.progress.progress * 100) }}%</span></div>
        <div v-if="apollo.progress.running" class="h-1.5 rounded-full overflow-hidden bg-muted"><div class="h-full bg-primary transition-[width]" :style="{ width: `${apollo.progress.progress * 100}%` }" /></div>
      </div>
      <p v-if="apollo.error || apollo.progress.error" role="alert" class="text-xs text-destructive break-words">{{ apollo.error || apollo.progress.error }}</p>
      <div v-if="result" class="flex flex-col gap-3">
        <p class="break-all select-text text-xs text-muted-foreground">{{ result.path }}</p>
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" @click="player.play(apollo.sample!)"><Play />{{ t('apollo.original') }}</Button>
          <Button variant="outline" size="sm" @click="player.play(result)"><Play />{{ t('apollo.result') }}</Button>
          <Button variant="ghost" size="icon-sm" :aria-label="t('apollo.edit')" @click="edit"><Pencil /></Button>
          <Button variant="ghost" size="icon-sm" :aria-label="t('apollo.reveal')" @click="revealItemInDir(result.path)"><FolderOpen /></Button>
        </div>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="apollo.open = false">{{ t('apollo.close') }}</Button>
        <Button v-if="apollo.progress.running" variant="outline" :disabled="apollo.cancelling" @click="apollo.cancel">{{ t('apollo.stop') }}</Button>
        <Button v-else :disabled="!apollo.sample || apollo.starting" @click="apollo.start">{{ t(apollo.ready ? 'apollo.start' : 'apollo.downloadStart') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
