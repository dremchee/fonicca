<script setup lang="ts">
import { computed, ref, useTemplateRef } from 'vue'
import { useEventListener, useResizeObserver } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { SAMPLE_DRAG_TYPE, snapProjectTime } from '@/project'
import { useProjectStore } from '@/stores/project'
import ProjectClip from './ProjectClip.vue'
import ProjectTrackHeader from './ProjectTrackHeader.vue'
const props = defineProps<{ scale: number, snap: boolean }>()
const project = useProjectStore()
const { t } = useI18n()
const dragOver = ref<string | null>(null)
const viewport = useTemplateRef<HTMLElement>('viewport')
const available = ref(0)
useResizeObserver(viewport, entries => available.value = entries[0].contentRect.width)
const beat = computed(() => 60 / project.document.bpm)
const seconds = computed(() => Math.min(600, Math.max(16 * beat.value, project.duration + 4 * beat.value, (available.value - 168) / props.scale)))
const width = computed(() => seconds.value * props.scale)
const bars = computed(() => Array.from({ length: Math.ceil(seconds.value / (beat.value * 4)) }, (_, i) => i))
const gridStyle = computed(() => ({ width: `${width.value}px`, backgroundSize: `${beat.value * props.scale}px 100%, ${beat.value * props.scale / 4}px 100%`, backgroundImage: 'linear-gradient(to right, var(--border) 1px, transparent 1px), linear-gradient(to right, color-mix(in srgb, var(--border) 40%, transparent) 1px, transparent 1px)' }))
function time(e: MouseEvent, snap = props.snap !== e.altKey) { return snapProjectTime((e.clientX - (e.currentTarget as HTMLElement).getBoundingClientRect().left) / props.scale, project.document.bpm, snap) }
function seek(e: MouseEvent) { project.seek(time(e)) }
function drop(e: DragEvent, track?: string) {
  e.preventDefault(); dragOver.value = null
  try {
    const ids: unknown = JSON.parse(e.dataTransfer?.getData(SAMPLE_DRAG_TYPE) ?? '')
    if (Array.isArray(ids) && ids.every(id => Number.isSafeInteger(id))) void project.addSamples(ids, track, time(e, props.snap))
  }
  catch { /* Ignore unrelated files dragged into the window. */ }
}
useEventListener(window, 'project:file-drop', (event: Event) => {
  const { paths, x, trackId } = (event as CustomEvent<{ paths: string[], x: number, trackId?: string }>).detail
  const lane = trackId ? document.querySelector<HTMLElement>(`[data-project-track="${CSS.escape(trackId)}"]`) : null
  const start = lane ? snapProjectTime((x - lane.getBoundingClientRect().left) / props.scale, project.document.bpm, props.snap) : project.cursor
  void project.addPaths(paths, trackId, start)
})
</script>

<template>
  <div ref="viewport" class="flex-1 min-h-0 overflow-auto border-y border-border" :aria-label="t('project.title')">
    <div :style="{ minWidth: `${width + 168}px` }" class="relative min-h-full">
      <div class="flex sticky top-0 z-30 bg-background h-8">
        <div class="w-48 shrink-0 sticky left-0 z-40 bg-background border-r border-b border-border px-3 flex items-center text-xs text-muted-foreground">4/4</div>
        <div class="relative h-8 border-b border-border cursor-crosshair" :style="gridStyle" @click="seek">
          <span v-for="bar in bars" :key="bar" class="absolute top-1 text-[11px] text-muted-foreground select-none pl-1" :style="{ left: `${bar * beat * 4 * scale}px` }">{{ bar + 1 }}</span>
        </div>
      </div>
      <div v-for="track in project.document.tracks" :key="track.id" class="flex">
        <ProjectTrackHeader :track="track" @update="project.updateTrack" @remove="id => project.edit(p => p.tracks = p.tracks.filter(t => t.id !== id))" />
        <div
          :data-project-track="track.id" class="relative h-24 shrink-0 border-b border-border"
          :class="{ 'bg-primary/5': dragOver === track.id, 'opacity-50': track.mute || (project.document.tracks.some(t => t.solo) && !track.solo) }" :style="gridStyle"
          @click="seek" @dragover.prevent="dragOver = track.id" @dragleave="dragOver = null" @drop="drop($event, track.id)"
        >
          <ProjectClip v-for="clip in track.clips" :key="clip.id" :clip="clip" :track-id="track.id" :scale="scale" :bpm="project.document.bpm" :snap="snap" :selected="project.selectedId === clip.id" @select="id => project.selectedId = id" @move="project.moveClip" @remove="project.deleteClip" @duplicate="project.duplicateClip" />
        </div>
      </div>
      <div class="flex min-h-32">
        <div class="w-48 shrink-0 sticky left-0 z-20 bg-background border-r border-border" />
        <div class="relative flex-1 text-xs text-muted-foreground p-4" :class="{ 'bg-primary/5': dragOver === 'new' }" @dragover.prevent="dragOver = 'new'" @dragleave="dragOver = null" @drop="drop($event)">
          {{ t('project.drop') }}
        </div>
      </div>
      <div class="absolute top-0 bottom-0 w-px bg-primary pointer-events-none z-10" :style="{ left: `${168 + project.cursor * scale}px` }"><div class="w-2 h-2 -translate-x-1 bg-primary" /></div>
    </div>
  </div>
</template>
