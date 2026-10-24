<script setup lang="ts">
import type { SliceBy, Span } from '@/stores/editor'
import { Slice } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useLocalStorage, watchDebounced } from '@vueuse/core'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { formatNumber } from '@/format'
import { useEditorStore } from '@/stores/editor'

// Cuts the selection (or the whole file) into slices and saves them as new samples.
// While open, the cuts are shown on the wave through `preview`.
const props = defineProps<{
  /** Frames per beat of the editor's bar grid; 0 without a tempo. */
  beatFrames: number
  /** Where bar 1 starts, in frames. */
  downbeat: number
  /** Move cuts to zero crossings. */
  zero: boolean
}>()
const preview = defineModel<Span[]>('preview', { required: true })

const { t } = useI18n()
const editor = useEditorStore()
const open = ref(false)
const exporting = ref(false)

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
watchDebounced(
  [open, by, target, () => editor.revision, () => props.zero],
  async () => {
    const req = ++request
    if (!open.value || !by.value || !editor.open) {
      preview.value = []
      return
    }
    try {
      const spans = await editor.slices(target.value.start, target.value.end, by.value, props.zero)
      if (req === request)
        preview.value = spans
    }
    catch (e) {
      console.error(e)
    }
  },
  { debounce: 120, deep: true },
)

async function exportSlices() {
  exporting.value = true
  try {
    const r = await editor.exportSlices(preview.value)
    toast.success(t('editor.slice.exported', { n: formatNumber(r.count) }, r.count), {
      description: r.inLibrary ? undefined : t('editor.slice.notInLibrary'),
      action: { label: t('editor.slice.reveal'), onClick: () => revealItemInDir(r.dir) },
    })
    open.value = false
  }
  catch (e) {
    toast.error(String(e))
  }
  finally {
    exporting.value = false
  }
}
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

          <div class="flex flex-col gap-1.5">
            <Button size="sm" :disabled="!preview.length || exporting" @click="exportSlices()">
              {{ t('editor.slice.export', { n: formatNumber(preview.length) }, preview.length) }}
            </Button>
            <p class="text-xs text-muted-foreground text-center">
              {{ t('editor.slice.where', { name: editor.sample?.name ?? '' }) }}
            </p>
          </div>
        </PopoverContent>
      </Popover>
    </span>
  </Hint>
</template>
