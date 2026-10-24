<script setup lang="ts">
import { Pause, Play, Square, Magnet, Undo2, Redo2, Plus, ZoomIn, ZoomOut } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { useProjectStore } from '@/stores/project'
const scale = defineModel<number>('scale', { required: true })
const snap = defineModel<boolean>('snap', { required: true })
const emit = defineEmits<{ add: [] }>()
const project = useProjectStore()
const { t } = useI18n()
function value(e: Event) { return Number((e.target as HTMLInputElement).value) }
</script>

<template>
  <div class="flex flex-wrap items-center gap-2 px-4 py-2 border-t border-border">
    <Button size="icon-sm" :variant="project.playing ? 'secondary' : 'default'" :disabled="!project.duration || project.busy" :aria-label="t('project.play')" :title="t('project.play')" @click="project.play"><component :is="project.playing ? Pause : Play" /></Button>
    <Button variant="ghost" size="icon-sm" :aria-label="t('project.stop')" :title="t('project.stop')" @click="project.stop(); project.cursor = 0"><Square /></Button>
    <label class="text-xs text-muted-foreground flex items-center gap-1">{{ t('project.cursor') }}<input class="w-20 bg-transparent border rounded px-1 h-7 tabular text-foreground" type="number" min="0" :max="project.duration" step="0.01" :value="project.cursor.toFixed(2)" @change="project.seek(value($event))"></label>
    <label class="text-xs text-muted-foreground flex items-center gap-1">{{ t('project.bpm') }}<input class="w-14 bg-transparent border rounded px-1 h-7 text-foreground" type="number" min="30" max="300" :value="project.document.bpm" @change="project.edit(p => p.bpm = value($event))"></label>
    <Button variant="ghost" size="icon-sm" :aria-label="t('project.snap')" :aria-pressed="snap" :title="t('project.snap')" :class="{ 'text-primary': snap }" @click="snap = !snap"><Magnet /></Button>
    <Button variant="ghost" size="icon-sm" :disabled="!project.past.length" :aria-label="t('project.undo')" :title="t('project.undo')" @click="project.undo"><Undo2 /></Button>
    <Button variant="ghost" size="icon-sm" :disabled="!project.future.length" :aria-label="t('project.redo')" :title="t('project.redo')" @click="project.redo"><Redo2 /></Button>
    <span class="flex-1" />
    <Button variant="ghost" size="icon-sm" :disabled="scale <= 10" :aria-label="`${t('project.zoom')} −`" @click="scale = Math.max(10, scale / 1.5)"><ZoomOut /></Button>
    <Button variant="ghost" size="icon-sm" :disabled="scale >= 400" :aria-label="`${t('project.zoom')} +`" @click="scale = Math.min(400, scale * 1.5)"><ZoomIn /></Button>
    <Button variant="outline" size="sm" :disabled="project.document.tracks.length >= 32" @click="project.addTrack"><Plus />{{ t('project.addTrack') }}</Button>
    <Button size="sm" @click="emit('add')"><Plus />{{ t('project.addSamples') }}</Button>
  </div>
</template>
