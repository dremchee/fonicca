<script setup lang="ts">
import { useTagColors } from '@/composables/tagColors'
import type { Sample, TrackTags } from '@/api'
import { Info, Music2, Pencil } from '@lucide/vue'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { openTrackInfo, trackInfoFor } from '@/composables/trackInfo'
import { categoryLabel, formatBpm, formatChannels, formatDb, formatDuration, formatRate, formatSize, kindLabel } from '@/format'

const props = withDefaults(defineProps<{
  sample: Sample
  tags: TrackTags | null
  coverUrl: string | null
  side?: 'top' | 'bottom'
  sideOffset?: number
}>(), { side: 'top', sideOffset: 8 })

const { t } = useI18n()
const { tagStyle } = useTagColors()
const visible = ref(false)
const coverFailed = ref(false)
watch(() => props.coverUrl, () => coverFailed.value = false)
const open = computed({
  get: () => visible.value && !trackInfoFor.value,
  set: value => visible.value = value,
})
watch(() => props.sample.id, () => visible.value = false)

let editAfterClose = false
function editMetadata() {
  editAfterClose = true
  open.value = false
}
function onCloseAutoFocus(event: Event) {
  if (!editAfterClose)
    return
  event.preventDefault()
  editAfterClose = false
  setTimeout(() => openTrackInfo(props.sample), 0)
}

const metadata = computed(() => [
  { label: t('trackInfo.fields.artist'), value: props.tags?.artist },
  { label: t('trackInfo.fields.album'), value: props.tags?.album },
  { label: t('trackInfo.fields.year'), value: props.tags?.year },
  { label: t('trackInfo.fields.genre'), value: props.tags?.genre },
].filter(row => row.value != null && row.value !== ''))

const details = computed(() => {
  const s = props.sample
  return [
    { label: t('list.columns.length'), value: formatDuration(s.duration) },
    { label: t('list.columns.type'), value: s.ext.toUpperCase() },
    { label: t('trackInfo.fields.sampleRate'), value: formatRate(s.sampleRate) },
    { label: t('trackInfo.fields.channels'), value: formatChannels(s.channels) },
    { label: t('trackInfo.fields.bitDepth'), value: s.bitDepth ? t('common.units.bit', { n: s.bitDepth }) : null },
    { label: t('trackInfo.fields.size'), value: formatSize(s.size) },
    { label: t('list.columns.category'), value: s.category ? categoryLabel(s.category) : null },
    { label: t('list.columns.kind'), value: s.kind ? kindLabel(s.kind) : null },
    { label: t('player.samplePeak'), value: s.peakDb != null ? formatDb(s.peakDb) : null },
    { label: t('player.integratedLoudness'), value: s.lufs != null ? formatDb(s.lufs, 'LUFS') : null },
    { label: 'BPM', value: s.bpm != null ? formatBpm(s.bpm) : null },
    { label: t('list.columns.key'), value: s.key },
  ].filter(row => row.value != null)
})
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <slot name="trigger">
        <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t('trackInfo.title')">
          <Info />
        </Button>
      </slot>
    </PopoverTrigger>
    <PopoverContent
      :side="side" align="end" :side-offset="sideOffset" :collision-padding="12" @close-auto-focus="onCloseAutoFocus"
      class="z-50 w-[560px] max-w-[calc(100vw-24px)] max-h-[var(--reka-popover-content-available-height)] overflow-y-auto rounded-md border bg-popover p-4 text-popover-foreground shadow-md outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
    >
      <div class="sample-info-layout">
        <div class="cover max-w-full rounded-sm overflow-hidden bg-muted flex items-center justify-center">
          <img v-if="coverUrl && !coverFailed" :src="coverUrl" :alt="t('trackInfo.cover')" width="192" height="192" class="size-full object-contain" @error="coverFailed = true">
          <div v-else class="flex flex-col items-center gap-3 text-muted-foreground">
            <Music2 class="size-10" aria-hidden="true" />
            <span class="text-xs">{{ t('trackInfo.noCover') }}</span>
          </div>
        </div>
        <div class="min-w-0">
          <div class="text-sm font-semibold [overflow-wrap:anywhere]">
            {{ tags?.title || sample.name }}
          </div>
          <div v-if="tags?.title" class="mt-1 text-xs text-muted-foreground [overflow-wrap:anywhere]">
            {{ sample.name }}.{{ sample.ext }}
          </div>
          <dl v-if="metadata.length" class="mt-3 flex flex-col gap-1 text-xs">
            <div v-for="row in metadata" :key="row.label" class="grid grid-cols-[5rem_1fr] gap-2">
              <dt class="text-muted-foreground">{{ row.label }}</dt>
              <dd class="min-w-0 [overflow-wrap:anywhere]">{{ row.value }}</dd>
            </div>
          </dl>
          <dl class="mt-3 grid grid-cols-2 gap-x-4 gap-y-2 border-t pt-3 text-xs">
            <div v-for="row in details" :key="row.label" class="min-w-0">
              <dt class="text-muted-foreground">{{ row.label }}</dt>
              <dd class="mt-0.5 tabular-nums">{{ row.value }}</dd>
            </div>
          </dl>
          <div v-if="sample.tags.length" class="mt-3 flex flex-wrap gap-1">
            <Badge v-for="tag in sample.tags" :key="tag.id" variant="secondary" :style="tagStyle(tag)" class="tag-color max-w-full whitespace-normal [overflow-wrap:anywhere]">{{ tag.name }}</Badge>
          </div>
          <p class="mt-3 text-[11px] text-muted-foreground [overflow-wrap:anywhere]">{{ sample.path }}</p>
        </div>
      </div>
      <div class="mt-3 flex items-center justify-end border-t pt-3">
        <Button variant="outline" size="sm" class="gap-2" @click="editMetadata">
          <Pencil /> {{ t('trackInfo.editMetadata') }}
        </Button>
      </div>
    </PopoverContent>
  </Popover>
</template>

<style scoped>
.sample-info-layout {
  display: grid;
  grid-template-columns: 192px minmax(0, 1fr);
  align-items: start;
  gap: 16px;
}
.cover {
  width: 192px;
  height: 192px;
}
@media (max-width: 560px) {
  .sample-info-layout {
    grid-template-columns: minmax(0, 1fr);
  }
  .cover {
    justify-self: center;
  }
}
</style>
