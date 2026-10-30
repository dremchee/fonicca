<script setup lang="ts">
import type { Marker, SliceBy, Span } from '@/stores/editor'
import { Slice } from '@lucide/vue'
import { useLocalStorage, watchDebounced } from '@vueuse/core'
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { formatNumber } from '@/format'
import { useEditorStore } from '@/stores/editor'

// Places slice markers over the selection (or the whole file): on the bar grid, at
// hits, or around silence. While open, the markers it would place show on the wave
// through `preview`; they can be edited afterwards, and Save regions saves them.
const props = defineProps<{
  /** Frames per beat of the editor's bar grid; 0 without a tempo. */
  beatFrames: number
  /** Where bar 1 starts, in frames. */
  downbeat: number
  /** Move cuts to zero crossings. */
  zero: boolean
}>()
const preview = defineModel<Marker[]>('preview', { required: true })

const { t } = useI18n()
const editor = useEditorStore()
const open = ref(false)
// What the backend found for the current settings.
const found = shallowRef<Span[]>([])

type Mode = SliceBy['mode']
const mode = useLocalStorage<Mode>('editor.slice.mode', 'transients')
// Slice length on the grid, in beats.
const division = useLocalStorage('editor.slice.division', 1)
const sensitivity = useLocalStorage('editor.slice.sensitivity', 50)
const minMs = useLocalStorage('editor.slice.minMs', 60)
const thresholdDb = useLocalStorage('editor.slice.thresholdDb', -50)
const minSilenceMs = useLocalStorage('editor.slice.minSilenceMs', 100)

const divisions = [
  { beats: 0.25, label: '1/16' },
  { beats: 0.5, label: '1/8' },
  { beats: 1, label: '1/4' },
  { beats: 2, label: '1/2' },
  { beats: 4, label: 'editor.slice.bars', n: 1 },
  { beats: 8, label: 'editor.slice.bars', n: 2 },
  { beats: 16, label: 'editor.slice.bars', n: 4 },
]
const divisionLabel = (d: typeof divisions[number]) => (d.n ? t(d.label, { n: d.n }, d.n) : d.label)

const target = computed(() => editor.selection ?? { start: 0, end: editor.state?.frames ?? 0 })

/**
 * Slices as markers: one at each slice start, and a skipped one where a slice ends
 * before the next begins (the silence between). Without markers so far, what lies
 * outside the target is skipped, so only the target's slices get saved.
 */
function asMarkers(spans: Span[], keepOutside: boolean): Marker[] {
  const { start, end } = target.value
  const frames = editor.state?.frames ?? 0
  const out: Marker[] = []
  if (spans.length && spans[0].start > start)
    out.push({ frame: start, skip: true })
  spans.forEach((s, i) => {
    out.push({ frame: s.start, skip: false })
    if ((spans[i + 1]?.start ?? end) > s.end)
      out.push({ frame: s.end, skip: true })
  })
  if (!keepOutside) {
    if (start > 0)
      out.unshift({ frame: 0, skip: true })
    if (end < frames)
      out.push({ frame: end, skip: true })
  }
  else if (end < frames && !out.some(m => m.frame === end)) {
    // What follows the target goes on as it was.
    out.push({ frame: end, skip: editor.regions.find(r => end >= r.start && end < r.end)?.skip ?? false })
  }
  return out
}

/** Puts the found markers in: replacing those inside the target, or added to them. */
function place(replace: boolean) {
  const old = editor.state?.markers ?? []
  const { start, end } = target.value
  const kept = replace ? old.filter(m => m.frame < start || m.frame >= end) : old
  const added = asMarkers(found.value, old.length > 0)
  // The added markers win where both sit on one frame.
  editor.setMarkers([...kept.filter(m => !added.some(a => a.frame === m.frame)), ...added])
  open.value = false
}

const regionCount = computed(() => editor.regions.filter(r => !r.skip).length)

const by = computed<SliceBy | null>(() => {
  switch (mode.value) {
    case 'grid':
      return props.beatFrames > 0 ? { mode: 'grid', origin: props.downbeat, step: division.value * props.beatFrames } : null
    case 'transients':
      return { mode: 'transients', sensitivity: sensitivity.value / 100, minMs: minMs.value }
    default:
      return { mode: 'silence', thresholdDb: thresholdDb.value, minSilenceMs: minSilenceMs.value }
  }
})

let request = 0
let disposed = false
onBeforeUnmount(() => { disposed = true; request++; preview.value = [] })
const inputs = () => [open.value, by.value, target.value, editor.revision, props.zero]
// Invalidate immediately; the next request starts only after the debounce. An
// older response can otherwise arrive in that gap and enable stale markers.
watch(inputs, () => {
  request++
  found.value = []
  preview.value = []
}, { deep: true, flush: 'sync' })
watchDebounced(
  inputs,
  async () => {
    if (disposed) return
    const req = ++request
    if (!open.value || !by.value || !editor.open) {
      found.value = []
      preview.value = []
      return
    }
    try {
      const spans = await editor.slices(target.value.start, target.value.end, by.value, props.zero)
      if (req === request) {
        found.value = spans
        preview.value = asMarkers(spans, true)
      }
    }
    catch (e) {
      if (!disposed && req === request) console.error(e)
    }
  },
  { debounce: 120, deep: true },
)

</script>

<template>
  <!-- The tooltip wraps the whole popover: inside it, it would take the popup's anchor. -->
  <Hint :text="open ? '' : t('editor.slice.hint')">
    <span class="inline-flex">
      <Popover v-model:open="open">
        <PopoverTrigger as-child>
          <Button variant="ghost" size="icon-sm" class="data-[state=open]:bg-accent" :aria-label="t('editor.slice.title')">
            <Slice />
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" class="w-72 flex flex-col gap-4 text-sm">
          <div class="flex items-baseline justify-between gap-2">
            <span class="font-medium">{{ t('editor.slice.title') }}</span>
            <span class="text-xs text-muted-foreground">
              {{ editor.selection ? t('editor.slice.ofSelection') : t('editor.slice.ofFile') }}
            </span>
          </div>
          <ToggleGroup
            :model-value="mode" type="single" variant="outline" size="sm" class="w-full"
            @update:model-value="$event && (mode = $event as Mode)"
          >
            <ToggleGroupItem value="grid" class="flex-1">
              {{ t('editor.slice.grid') }}
            </ToggleGroupItem>
            <ToggleGroupItem value="transients" class="flex-1">
              {{ t('editor.slice.transients') }}
            </ToggleGroupItem>
            <ToggleGroupItem value="silence" class="flex-1">
              {{ t('editor.slice.silence') }}
            </ToggleGroupItem>
          </ToggleGroup>

          <template v-if="mode === 'grid'">
            <p v-if="!beatFrames" class="text-muted-foreground">
              {{ t('editor.slice.needBpm') }}
            </p>
            <div v-else class="flex items-center justify-between gap-3">
              <span class="text-muted-foreground">{{ t('editor.slice.every') }}</span>
              <!-- SelectValue keeps the text it was mounted with: spell the label out. -->
              <Select :model-value="String(division)" @update:model-value="division = Number($event)">
                <SelectTrigger size="sm" class="w-28" :aria-label="t('editor.slice.every')">
                  <SelectValue>{{ divisionLabel(divisions.find(d => d.beats === division) ?? divisions[2]) }}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="d in divisions" :key="d.beats" :value="String(d.beats)">
                    {{ divisionLabel(d) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </template>

          <template v-else-if="mode === 'transients'">
            <label class="flex flex-col gap-2">
              <span class="flex justify-between text-muted-foreground">
                {{ t('editor.slice.sensitivity') }}<span class="tabular">{{ sensitivity }}%</span>
              </span>
              <Slider :model-value="[sensitivity]" :max="100" :step="1" :aria-label="t('editor.slice.sensitivity')" @update:model-value="sensitivity = $event?.[0] ?? 50" />
            </label>
            <label class="flex flex-col gap-2">
              <span class="flex justify-between text-muted-foreground">
                {{ t('editor.slice.minLength') }}<span class="tabular">{{ t('editor.slice.ms', { n: minMs }) }}</span>
              </span>
              <Slider :model-value="[minMs]" :min="20" :max="500" :step="10" :aria-label="t('editor.slice.minLength')" @update:model-value="minMs = $event?.[0] ?? 60" />
            </label>
          </template>

          <template v-else>
            <label class="flex flex-col gap-2">
              <span class="flex justify-between text-muted-foreground">
                {{ t('editor.slice.threshold') }}<span class="tabular">{{ thresholdDb }} {{ t('common.units.dB') }}</span>
              </span>
              <Slider :model-value="[thresholdDb]" :min="-80" :max="-20" :step="1" :aria-label="t('editor.slice.threshold')" @update:model-value="thresholdDb = $event?.[0] ?? -50" />
            </label>
            <label class="flex flex-col gap-2">
              <span class="flex justify-between text-muted-foreground">
                {{ t('editor.slice.minSilence') }}<span class="tabular">{{ t('editor.slice.ms', { n: minSilenceMs }) }}</span>
              </span>
              <Slider :model-value="[minSilenceMs]" :min="20" :max="1000" :step="10" :aria-label="t('editor.slice.minSilence')" @update:model-value="minSilenceMs = $event?.[0] ?? 100" />
            </label>
          </template>

          <div class="flex gap-2">
            <Button size="sm" class="flex-1" :disabled="!found.length" @click="place(true)">
              {{ t('editor.slice.place', { n: formatNumber(found.length) }, found.length) }}
            </Button>
            <Button v-if="editor.state?.markers.length" variant="outline" size="sm" :disabled="!found.length" @click="place(false)">
              {{ t('editor.slice.add') }}
            </Button>
          </div>
          <div class="-mx-4 -mb-4 px-4 py-3 border-t border-border bg-muted/40 text-xs text-muted-foreground flex flex-col gap-1">
            <div class="flex items-center justify-between gap-2">
              <span>{{ t('editor.slice.regions', { n: formatNumber(regionCount) }, regionCount) }}</span>
              <Button
                v-if="editor.state?.markers.length" variant="ghost" size="xs" class="-my-1 -mr-2 text-muted-foreground"
                @click="editor.setMarkers([])"
              >
                {{ t('editor.slice.clear') }}
              </Button>
            </div>
            <p>{{ t('editor.slice.help') }}</p>
          </div>
        </PopoverContent>
      </Popover>
    </span>
  </Hint>
</template>
