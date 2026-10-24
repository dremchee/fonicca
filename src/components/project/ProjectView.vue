<script setup lang="ts">
import { onKeyStroke } from '@vueuse/core'
import { onBeforeUnmount, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import LevelMeter from '@/components/LevelMeter.vue'
import { useProjectStore } from '@/stores/project'
import ProjectTimeline from './ProjectTimeline.vue'
import ProjectTransport from './ProjectTransport.vue'
import ProjectClipInspector from './ProjectClipInspector.vue'
import ProjectSamplePicker from './ProjectSamplePicker.vue'
const project = useProjectStore()
const { t } = useI18n()
const scale = ref(80)
const snap = ref(true)
const picker = ref(false)
function typing(e: KeyboardEvent) { return (e.target as Element)?.closest('input, textarea, button, [role="dialog"], [role="alertdialog"], [role="menu"]') }
onKeyStroke(' ', e => { if (!typing(e)) { e.preventDefault(); void project.play() } })
onKeyStroke('Escape', e => { if (!typing(e)) project.stop() })
onKeyStroke(['z', 'я'], e => { if ((e.metaKey || e.ctrlKey) && !typing(e)) { e.preventDefault(); e.shiftKey ? project.redo() : project.undo() } })
onKeyStroke(['s', 'ы'], e => { if ((e.metaKey || e.ctrlKey) && !typing(e)) { e.preventDefault(); void project.saveProject() } })
onBeforeUnmount(project.stop)
</script>

<template>
  <section class="flex flex-col h-full min-h-0 min-w-0">
    <header class="flex flex-wrap items-center gap-2 px-4 py-3">
      <input class="bg-transparent text-sm font-medium min-w-0 flex-1 outline-none focus-visible:ring-1 focus-visible:ring-ring rounded" :value="project.document.name" :aria-label="t('project.title')" maxlength="120" @change="project.edit(p => p.name = ($event.target as HTMLInputElement).value)">
      <span v-if="project.dirty" class="text-muted-foreground" :title="t('project.save')">•</span>
      <Button variant="ghost" size="sm" @click="project.newProject">{{ t('project.new') }}</Button>
      <Button variant="ghost" size="sm" @click="project.openProject">{{ t('project.open') }}</Button>
      <Button variant="outline" size="sm" @click="project.saveProject">{{ t('project.save') }}</Button>
      <Button :disabled="!project.duration || project.exporting" size="sm" @click="project.exportMix">{{ t(project.exporting ? 'project.exporting' : 'project.export') }}</Button>
    </header>
    <ProjectTransport v-model:scale="scale" v-model:snap="snap" @add="picker = true" />
    <p v-if="project.error" role="alert" class="px-4 py-2 text-xs text-destructive">{{ project.error }}</p>
    <p v-if="project.busy" role="status" class="px-4 py-2 text-xs text-muted-foreground">{{ t('project.preparing') }}</p>
    <div v-if="!project.document.tracks.length" class="px-4 py-8 text-center text-sm">
      <p class="font-medium">{{ t('project.empty') }}</p><p class="mt-2 text-xs text-muted-foreground">{{ t('project.emptyHint') }}</p>
    </div>
    <div class="flex flex-1 min-h-32 min-w-0"><ProjectTimeline :scale="scale" :snap="snap" /><LevelMeter project :active="project.playing" /></div>
    <ProjectClipInspector />
    <footer class="px-4 py-2 shrink-0 border-t border-border flex flex-wrap items-center gap-3 text-[11px] text-muted-foreground">
      <p class="flex-1 min-w-40" :title="`${t('project.dragHint')}\n${t('project.filesHint')}`">{{ t('project.timingHint') }}</p>
      <label class="flex items-center gap-2">{{ t('project.master') }}<input type="number" min="-60" max="0" step="1" class="w-14 bg-transparent border rounded px-1 h-6 text-foreground" :value="project.document.masterDb" @change="project.edit(p => p.masterDb = Number(($event.target as HTMLInputElement).value))"></label>
      <span>48 kHz · 32-bit float</span>
    </footer>
    <ProjectSamplePicker v-model="picker" />
  </section>
</template>
