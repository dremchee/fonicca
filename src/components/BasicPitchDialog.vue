<script setup lang="ts">
import { FolderOpen } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { formatNumber, formatSize } from '@/format'
import { useBasicPitchStore } from '@/stores/basicPitch'
const pitch = useBasicPitchStore()
const { t } = useI18n()
async function reveal() {
  if (!pitch.result?.path) return
  try { await revealItemInDir(pitch.result.path) }
  catch (error) { pitch.error = String(error) }
}
</script>
<template>
  <Dialog v-model:open="pitch.open">
    <DialogContent class="sm:max-w-lg max-h-[90vh] overflow-y-auto">
      <DialogHeader><DialogTitle>{{ t('basicPitch.title') }}</DialogTitle><DialogDescription>{{ t('basicPitch.description') }}</DialogDescription></DialogHeader>
      <div class="flex flex-col gap-1">
        <p class="truncate text-sm font-medium">{{ pitch.progress.running ? pitch.progress.name : pitch.sample?.name }}</p>
        <p v-if="!pitch.progress.running" class="text-xs text-muted-foreground">{{ t(`basicPitch.sources.${pitch.sourceKind}`) }}</p>
      </div>
      <p class="text-xs text-muted-foreground">{{ pitch.ready ? t('basicPitch.ready') : t('basicPitch.download', { size: formatSize(pitch.bytes) }) }}</p>
      <p class="text-xs text-muted-foreground">{{ t('basicPitch.hint') }}</p>
      <p class="text-xs text-muted-foreground">{{ t('basicPitch.limits') }}</p>
      <div v-if="pitch.progress.stage && (pitch.progress.running || pitch.current)" class="flex flex-col gap-2" aria-live="polite">
        <div class="flex justify-between text-xs"><span>{{ pitch.cancelling ? t('basicPitch.stopping') : t(`basicPitch.stages.${pitch.progress.stage}`) }}</span><span v-if="pitch.progress.running">{{ Math.round(pitch.progress.progress * 100) }}%</span></div>
        <div v-if="pitch.progress.running" class="h-1.5 rounded-full overflow-hidden bg-muted"><div class="h-full bg-primary transition-[width]" :style="{ width: `${pitch.progress.progress * 100}%` }" /></div>
      </div>
      <p v-if="pitch.error || (pitch.current && pitch.progress.error)" role="alert" class="text-xs text-destructive break-words">{{ pitch.error || (pitch.current && pitch.progress.error) }}</p>
      <div v-if="pitch.result" class="flex flex-col gap-2">
        <p class="text-sm">{{ t('basicPitch.result', { n: formatNumber(pitch.result.noteCount) }) }}</p>
        <p class="break-all select-text text-xs text-muted-foreground">{{ pitch.result.path }}</p>
        <Button variant="outline" size="sm" class="self-start" @click="reveal"><FolderOpen />{{ t('basicPitch.reveal') }}</Button>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="pitch.open = false">{{ t('basicPitch.close') }}</Button>
        <Button v-if="pitch.progress.running" variant="outline" :disabled="pitch.cancelling" @click="pitch.cancel">{{ t('basicPitch.stop') }}</Button>
        <Button v-else :disabled="!pitch.sample || pitch.starting" @click="pitch.start">{{ t(pitch.ready ? 'basicPitch.start' : 'basicPitch.downloadStart') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
