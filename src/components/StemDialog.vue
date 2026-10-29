<script setup lang="ts">
import { Pencil, Settings2 } from '@lucide/vue'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { useSettingsStore } from '@/stores/settings'
import { formatSize, shortPath } from '@/format'
import { useStemsStore } from '@/stores/stems'
import { stemNames, stemModelBytes } from '@/stem-models'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
const stems = useStemsStore()
const { t } = useI18n()
const settings = useSettingsStore()
const folder = computed(() => settings.settings.stemsFolder || stems.sample?.path.replace(/[/\\][^/\\]+$/, '') || '')
const names = computed(() => stemNames(stems.progress.mode))
function choose() { stems.open = false; settings.openDialog('analysis') }
async function edit(index: number) {
  try { await invoke('open_editor_window', { id: stems.progress.ids[index] }) }
  catch (e) { stems.error = String(e) }
}
</script>

<template>
  <Dialog v-model:open="stems.open">
    <DialogContent class="sm:max-w-lg max-h-[90vh] overflow-y-auto">
      <DialogHeader><DialogTitle>{{ t('stems.title') }}</DialogTitle><DialogDescription>{{ t('stems.description') }}</DialogDescription></DialogHeader>
      <p v-if="stems.sample" class="text-sm font-medium truncate" :title="stems.sample.path">{{ stems.sample.name }}</p>
      <div class="flex flex-col gap-2">
        <span class="text-xs font-medium">{{ t('stems.mode') }}</span>
        <ToggleGroup type="single" variant="outline" :model-value="stems.mode" :disabled="stems.progress.running || stems.starting" :aria-label="t('stems.mode')" @update:model-value="stems.setMode">
          <ToggleGroupItem value="two" class="flex-1">{{ t('stems.twoTracks') }}</ToggleGroupItem>
          <ToggleGroupItem value="four" class="flex-1">{{ t('stems.fourTracks') }}</ToggleGroupItem>
        </ToggleGroup>
        <p class="text-xs text-muted-foreground">{{ t(stems.mode === 'two' ? 'stems.twoDescription' : 'stems.fourDescription') }}</p>
      </div>
      <div class="flex flex-col gap-2">
        <label for="stem-model" class="text-xs font-medium">{{ t('stems.model') }}</label>
        <Select :model-value="stems.model" :disabled="stems.progress.running || stems.starting" @update:model-value="stems.setModel">
          <SelectTrigger id="stem-model"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectGroup><SelectItem v-for="model in stems.availableModels" :key="model" :value="model">
              {{ t(`stems.models.${model}`) }} · {{ formatSize(stemModelBytes[model]) }}
            </SelectItem></SelectGroup>
          </SelectContent>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t(`stems.modelDescriptions.${stems.model}`) }}</p>
        <p class="text-xs text-muted-foreground">{{ stems.ready ? t('stems.ready') : t('stems.download', { size: formatSize(stems.bytes) }) }}</p>
      </div>
      <div class="rounded-md border border-border p-3 flex flex-col gap-2">
        <span class="text-xs font-medium">{{ t('stems.folder') }}</span>
        <div class="flex items-center gap-2"><span class="min-w-0 flex-1 truncate text-xs text-muted-foreground" :title="folder">{{ folder ? shortPath(folder) : t('stems.noFolder') }}</span><Button size="sm" variant="outline" :disabled="stems.progress.running || stems.starting" @click="choose"><Settings2 />{{ t('stems.folderSettings') }}</Button></div>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('stems.limits') }}</p>
      <div v-if="stems.progress.stage" class="flex flex-col gap-2" aria-live="polite">
        <div class="text-xs text-muted-foreground flex justify-between gap-2"><span class="truncate">{{ stems.progress.name }}</span><span v-if="stems.progress.running">{{ Math.round(stems.progress.progress * 100) }}%</span></div>
        <div v-if="stems.progress.running" class="h-1.5 bg-muted rounded-full overflow-hidden"><div class="h-full bg-primary transition-[width]" :style="{ width: `${stems.progress.progress * 100}%` }" /></div>
        <p class="text-xs">{{ stems.cancelling ? t('stems.stopping') : t(`stems.stages.${stems.progress.stage}`) }}</p>
      </div>
      <p v-if="stems.error || stems.progress.error" role="alert" class="text-xs text-destructive break-words">{{ stems.error || stems.progress.error }}</p>
      <template v-if="stems.progress.paths.length && !stems.progress.running">
        <div class="divide-y divide-border text-xs"><div v-for="(path, i) in stems.progress.paths" :key="path" class="flex items-center gap-2 py-1.5"><span class="flex-1" :title="path">{{ t(`stems.instruments.${names[i]}`) }}</span><Button variant="ghost" size="icon-xs" :disabled="!stems.progress.ids[i]" :aria-label="`${t('stems.edit')} · ${t(`stems.instruments.${names[i]}`)}`" @click="edit(i)"><Pencil /></Button></div></div>
        <div class="flex flex-wrap gap-2"><Button variant="outline" size="sm" @click="revealItemInDir(stems.progress.paths[0])">{{ t('stems.reveal') }}</Button></div>
      </template>
      <DialogFooter>
        <Button variant="outline" @click="stems.open = false">{{ t('stems.close') }}</Button>
        <Button v-if="stems.progress.running" variant="outline" :disabled="stems.cancelling" @click="stems.cancel">{{ t('stems.stop') }}</Button>
        <Button v-else :disabled="!stems.sample || !folder || stems.starting" @click="stems.start()">{{ t(stems.ready ? 'stems.start' : 'stems.downloadStart') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
