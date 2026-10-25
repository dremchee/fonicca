<script setup lang="ts">
import type { Component } from 'vue'
import type { Sample } from '@/api'
import {
  AudioLines, AudioWaveform, CircleDot, Disc, Disc3, Drum, Drumstick,
  EighthNotes, Guitar, GuitarElectric, Hand, MicVocal, Piano, Sparkles, WavesHorizontal, Wind,
} from '@lucide/vue'
import { computed } from 'vue'
import { categoryLabel } from '@/format'

const props = defineProps<{ category?: string | null, stem?: Sample['stemKind'] }>()
const aliases: Record<string, string> = { drum: 'drums', vocals: 'vocal' }
const category = computed(() => {
  const id = props.stem ? props.stem === 'other' ? null : props.stem : props.category
  return id ? aliases[id] ?? id : null
})
const icons: Record<string, Component> = {
  kick: CircleDot, snare: Drum, clap: Hand, hihat: Disc3, cymbal: Disc,
  tom: Drum, percussion: Drumstick, drums: Drum,
  bass: GuitarElectric, synth: AudioLines, pad: WavesHorizontal, keys: Piano,
  guitar: Guitar, strings: EighthNotes, brass: Wind, vocal: MicVocal, fx: Sparkles,
}
const icon = computed(() => category.value ? icons[category.value] ?? AudioWaveform : AudioWaveform)
</script>

<template>
  <component :is="icon" aria-hidden="true" :stroke-width="1.75">
    <title v-if="category">{{ categoryLabel(category) }}</title>
  </component>
</template>
