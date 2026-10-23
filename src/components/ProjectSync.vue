<script setup lang="ts">
import { Metronome } from '@lucide/vue'
import { computed } from 'vue'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const settings = useSettingsStore()
const player = usePlayerStore()

const notes = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']
// Reka selects cannot hold null, so "no key" is a sentinel value.
const NONE = 'none'

const enabled = computed({
  get: () => settings.settings.syncEnabled,
  set: (syncEnabled: boolean) => settings.update({ syncEnabled }),
})

const key = computed({
  get: () => settings.settings.projectKey ?? NONE,
  set: (v: string) => settings.update({ projectKey: v === NONE ? null : v }),
})

function setBpm(e: Event) {
  const v = Number.parseFloat((e.target as HTMLInputElement).value)
  settings.update({ projectBpm: Number.isFinite(v) && v > 0 ? v : null })
}

const label = computed(() => {
  const s = settings.settings
  const parts = [s.projectBpm ? `${s.projectBpm}` : '', s.projectKey ?? ''].filter(Boolean)
  return parts.length ? parts.join(' · ') : 'Sync'
})

/** What happened to the sample playing now, e.g. "×0.94 · +2 st". */
const applied = computed(() => {
  const a = player.adjust
  if (!a || player.playingId === null || !player.syncTarget)
    return ''
  const parts = []
  if (Math.abs(a.stretch - 1) > 0.001)
    parts.push(`tempo ×${(1 / a.stretch).toFixed(2)}`)
  if (a.semitones)
    parts.push(`${a.semitones > 0 ? '+' : ''}${a.semitones} st`)
  return parts.join(' · ')
})
</script>

<template>
  <Popover>
    <Hint :text="applied ? `Playing adjusted: ${applied}` : 'Preview in the project tempo and key'">
      <PopoverTrigger as-child>
        <Button
          variant="ghost" size="sm"
          :class="enabled ? 'text-primary' : 'text-muted-foreground'"
          aria-label="Project tempo and key"
        >
          <Metronome />
          {{ enabled ? label : 'Sync' }}
        </Button>
      </PopoverTrigger>
    </Hint>
    <PopoverContent class="w-64 grid gap-3" align="end">
      <div class="flex items-center justify-between gap-2">
        <Label for="project-sync">Match project</Label>
        <Switch id="project-sync" :model-value="enabled" @update:model-value="enabled = $event" />
      </div>
      <p class="text-xs text-muted-foreground leading-relaxed">
        Loops play stretched to the project tempo, samples with a known key transposed to the project key.
      </p>
      <div class="flex items-center justify-between gap-2">
        <Label for="project-bpm" class="font-normal">Tempo</Label>
        <Input
          id="project-bpm" type="number" min="20" max="400" placeholder="BPM"
          class="h-8 w-24 num" :model-value="settings.settings.projectBpm ?? ''" @change="setBpm"
        />
      </div>
      <div class="flex items-center justify-between gap-2">
        <Label class="font-normal">Key</Label>
        <Select v-model="key">
          <SelectTrigger size="sm" class="w-24" aria-label="Project key">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="NONE">
              None
            </SelectItem>
            <SelectSeparator />
            <SelectGroup>
              <SelectLabel>Major</SelectLabel>
              <SelectItem v-for="n in notes" :key="n" :value="n">
                {{ n }}
              </SelectItem>
            </SelectGroup>
            <SelectSeparator />
            <SelectGroup>
              <SelectLabel>Minor</SelectLabel>
              <SelectItem v-for="n in notes" :key="`${n}m`" :value="`${n}m`">
                {{ n }}m
              </SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>
    </PopoverContent>
  </Popover>
</template>

<style scoped>
.num::-webkit-inner-spin-button {
  display: none;
}
</style>
