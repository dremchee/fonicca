<script setup lang="ts">
import type { ProjectTrack } from '@/project'
import { Trash2 } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
const props = defineProps<{ track: ProjectTrack }>()
const emit = defineEmits<{ update: [id: string, values: Partial<ProjectTrack>], remove: [id: string] }>()
const { t } = useI18n()
function value(e: Event) { return Number((e.target as HTMLInputElement).value) }
</script>

<template>
  <div class="h-24 w-48 shrink-0 px-3 py-2 flex flex-col gap-1.5 border-r border-b border-border bg-background sticky left-0 z-20">
    <div class="flex items-center gap-1">
      <input class="min-w-0 w-full bg-transparent text-xs font-medium outline-none focus-visible:ring-1 focus-visible:ring-ring rounded" :value="track.name" :aria-label="t('project.track')" maxlength="120" @change="emit('update', track.id, { name: ($event.target as HTMLInputElement).value })">
      <Button variant="ghost" size="icon-xs" :aria-label="t('project.removeTrack')" @click="emit('remove', track.id)"><Trash2 class="size-3" /></Button>
    </div>
    <div class="flex items-center gap-1">
      <Button :variant="track.mute ? 'secondary' : 'outline'" size="xs" :aria-pressed="track.mute" :aria-label="t('project.mute')" @click="emit('update', track.id, { mute: !track.mute })">M</Button>
      <Button :variant="track.solo ? 'default' : 'outline'" size="xs" :aria-pressed="track.solo" :aria-label="t('project.solo')" @click="emit('update', track.id, { solo: !track.solo })">S</Button>
      <span class="text-[11px] tabular text-muted-foreground ml-auto">{{ track.gainDb }} dB</span>
    </div>
    <div class="flex gap-2">
      <input class="min-w-0 w-1/2 h-3 accent-primary" type="range" min="-60" max="12" step="1" :value="track.gainDb" :aria-label="t('project.gain')" @change="emit('update', track.id, { gainDb: value($event) })">
      <input class="min-w-0 w-1/2 h-3 accent-primary" type="range" min="-1" max="1" step="0.05" :value="track.pan" :aria-label="t('project.pan')" @change="emit('update', track.id, { pan: value($event) })">
    </div>
  </div>
</template>
