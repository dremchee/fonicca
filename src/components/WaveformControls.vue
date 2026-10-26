<script setup lang="ts">
import type { WaveformSettings } from '@/api'
import { ZoomIn, ZoomOut } from '@lucide/vue'
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import WaveformPreview from '@/components/WaveformPreview.vue'
import { Button } from '@/components/ui/button'
import { ColorPicker } from '@/components/ui/color-picker'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useSettingsStore } from '@/stores/settings'
import { waveformSettings } from '@/waveform-settings'

const props = defineProps<{ options: WaveformSettings, target: 'preview' | 'editor' }>()
const emit = defineEmits<{ change: [patch: Partial<WaveformSettings>] }>()
const { t } = useI18n()
const id = useId()
const settings = useSettingsStore()
const themeColor = computed(() => {
  const palette = settings.preview?.palette ?? settings.activeTheme[settings.mode]
  const hex = props.target === 'editor' ? palette.accent : palette.wave
  return hex.length === 4 ? `#${[...hex.slice(1)].map(c => c + c).join('')}` : hex
})
const draft = ref<Partial<WaveformSettings>>({})
const previewZoom = ref(1)
const previewPan = ref(0)
function zoomPreview(direction: number) {
  previewZoom.value = Math.max(1, Math.min(8, previewZoom.value * (direction > 0 ? 2 : 0.5)))
  if (previewZoom.value === 1) previewPan.value = 0
}
const value = computed(() => ({ ...waveformSettings(props.options), ...draft.value }))
watch(() => props.options, () => draft.value = {}, { deep: true })

const choices = [
  { key: 'waveStyle', label: 'style', items: [['peaks', 'peaks'], ['peaksRms', 'peaksRms'], ['bars', 'bars']] },
  { key: 'waveDensity', label: 'density', items: [['off', 'densityOff'], ['gaussian', 'densityGaussian'], ['laplace', 'densityLaplace'], ['flat', 'densityFlat']] },
  { key: 'waveScale', label: 'scale', items: [['linear', 'linear'], ['db', 'db']] },
  { key: 'waveDbRange', label: 'dbRange', items: [[40, '40 dB'], [60, '60 dB'], [90, '90 dB']] },
  { key: 'waveChannels', label: 'channels', items: [['split', 'split'], ['mix', 'mix']] },
  { key: 'waveColors', label: 'colors', items: [['theme', 'theme'], ['frequency', 'frequency']] },
  { key: 'waveThickness', label: 'thickness', items: [[1, '1 px'], [2, '2 px'], [3, '3 px']] },
] as const
const barControls = [
  { key: 'waveBarWidth', min: 1, max: 8, step: 0.5, scale: 1, unit: 'px' },
  { key: 'waveBarGap', min: 0, max: 6, step: 0.5, scale: 1, unit: 'px' },
  { key: 'waveBarRadius', min: 0, max: 100, step: 5, scale: 100, unit: '%' },
] as const
const opacityControls = ['wavePeakOpacity', 'waveRmsOpacity'] as const

const groups = [
  { id: 'appearance', keys: ['waveStyle', 'waveDensity', 'waveThickness'] },
  { id: 'amplitude', keys: ['waveScale', 'waveDbRange'] },
  { id: 'channelsColors', keys: ['waveChannels', 'waveColors'] },
] as const
function visible(key: string) {
  return !((key === 'waveDensity' || key === 'waveThickness') && value.value.waveStyle === 'bars')
    && !(key === 'waveDbRange' && value.value.waveScale !== 'db')
}
function groupRows(keys: readonly string[]) {
  return choices.filter(row => keys.includes(row.key) && visible(row.key))
}
function choiceLabel(item: readonly [string | number, string]) {
  return typeof item[0] === 'number' ? item[1] : t(`settings.waveform.${item[1]}`)
}
function option(key: typeof choices[number]['key'], next: unknown) {
  const row = choices.find(row => row.key === key)!
  const selected = row.items.find(item => String(item[0]) === next)
  if (selected)
    emit('change', { [key]: selected[0] })
}
function opacityDisabled() {
  return value.value.waveStyle === 'bars' || (value.value.waveDensity === 'off' && value.value.waveStyle !== 'peaksRms')
}
function numericOption(key: typeof opacityControls[number] | typeof barControls[number]['key'], next: number[] | undefined, commit = false, scale = 100) {
  if (!next?.length)
    return
  const patch = { [key]: next[0] / scale }
  if (commit) {
    draft.value = {}
    emit('change', patch)
  }
  else {
    draft.value = patch
  }
}
const colorLane = ref<number | null>(null)
const colorDraft = ref('#000000')
function setColorOpen(lane: number, open: boolean) {
  draft.value = {}
  colorLane.value = open ? lane : null
  if (open) colorDraft.value = value.value.waveChannelColors[lane] ?? themeColor.value
}
function previewColor(color: string) {
  colorDraft.value = color
  if (colorLane.value == null) return
  const colors = [...value.value.waveChannelColors] as WaveformSettings['waveChannelColors']
  colors[colorLane.value] = color
  draft.value = { waveChannelColors: colors }
}
function saveColor() {
  const colors = value.value.waveChannelColors
  draft.value = {}
  colorLane.value = null
  emit('change', { waveChannelColors: colors })
}
</script>

<template>
  <div class="wave-controls flex flex-col gap-6">
    <section class="wave-example sticky top-0 z-1 rounded-lg border border-border bg-card p-3 flex flex-col gap-3" :aria-label="t('settings.waveform.preview')">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h3 class="text-sm font-medium">{{ t('settings.waveform.preview') }}</h3>
        <div class="flex items-center gap-1" role="group" :aria-label="t('settings.waveform.zoom')">
          <Button variant="ghost" size="icon-xs" :disabled="previewZoom === 1" :aria-label="t('settings.waveform.zoomOut')" @click="zoomPreview(-1)"><ZoomOut class="size-4" /></Button>
          <Button variant="ghost" size="xs" class="w-10 tabular-nums font-normal" :aria-label="t('settings.waveform.resetZoom')" @click="previewZoom = 1; previewPan = 0">{{ Number(previewZoom.toFixed(1)) }}×</Button>
          <Button variant="ghost" size="icon-xs" :disabled="previewZoom === 8" :aria-label="t('settings.waveform.zoomIn')" @click="zoomPreview(1)"><ZoomIn class="size-4" /></Button>
        </div>
      </div>
      <WaveformPreview :options="value" :accent="props.target === 'editor'" :zoom="previewZoom" :pan="previewPan" @pan="previewPan = $event" @viewport="previewZoom = $event.zoom; previewPan = $event.pan" />
    </section>

    <div class="rounded-lg border border-border bg-card divide-y divide-border">
      <section v-for="group in groups" :key="group.id" class="flex flex-col gap-1 p-4" :aria-labelledby="`${id}-${group.id}`">
        <h3 :id="`${id}-${group.id}`" class="text-sm font-medium">{{ t(`settings.waveform.groups.${group.id}`) }}</h3>
        <div class="flex flex-col">
          <template v-for="row in groupRows(group.keys)" :key="row.key">
            <div class="option-row" :class="{ 'dependent-row': row.key === 'waveDbRange' }">
              <label :id="`${id}-${row.key}`" class="min-w-0 text-sm">{{ t(`settings.waveform.${row.label}`) }}</label>
              <ToggleGroup
                :model-value="String(value[row.key])" type="single" variant="outline" size="sm"
                class="option-control" :aria-labelledby="`${id}-${row.key}`"
                @update:model-value="option(row.key, $event)"
              >
                <ToggleGroupItem v-for="item in row.items" :key="item[0]" :value="String(item[0])" class="flex-1 px-2">
                  {{ choiceLabel(item) }}
                </ToggleGroupItem>
              </ToggleGroup>
            </div>
            <div v-if="row.key === 'waveStyle' && value.waveStyle === 'bars'" class="flex flex-col">
              <div v-for="control in barControls" :key="control.key" class="option-row dependent-row">
                <label :id="`${id}-${control.key}`" class="text-xs text-muted-foreground">{{ t(`settings.waveform.${control.key}`) }}</label>
                <div class="option-control flex items-center gap-3">
                  <Slider
                    :model-value="[value[control.key] * control.scale]" :min="control.min" :max="control.max" :step="control.step"
                    :aria-label="t(`settings.waveform.${control.key}`)"
                    @update:model-value="numericOption(control.key, $event, false, control.scale)"
                    @value-commit="numericOption(control.key, $event, true, control.scale)"
                  />
                  <output class="w-12 shrink-0 text-right text-xs tabular-nums text-muted-foreground">{{ Number((value[control.key] * control.scale).toFixed(1)) }} {{ control.unit }}</output>
                </div>
              </div>
            </div>
          </template>

          <template v-if="group.id === 'channelsColors'">
            <div v-if="value.waveColors === 'theme'" class="option-row">
              <div>
                <p :id="`${id}-channel-colors`" class="text-sm">{{ t('settings.waveform.channelColors') }}</p>
                <p class="mt-1 text-xs text-muted-foreground">{{ t('settings.waveform.channelColorsDesc') }}</p>
              </div>
              <div class="option-control flex items-center gap-2" role="group" :aria-labelledby="`${id}-channel-colors`">
                <Popover v-for="(_, lane) in value.waveChannels === 'split' ? 2 : 1" :key="lane" :open="colorLane === lane" @update:open="setColorOpen(lane, $event)">
                  <PopoverTrigger as-child>
                    <Button variant="outline" size="sm" class="flex-1 font-normal">
                      <span class="size-3 shrink-0 rounded-full" :style="{ backgroundColor: value.waveChannelColors[lane] ?? themeColor }" />
                      {{ t(`settings.waveform.${value.waveChannels === 'mix' ? 'monoColor' : lane === 0 ? 'leftColor' : 'rightColor'}`) }}
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent align="end" class="w-72 p-3">
                    <ColorPicker :model-value="colorDraft" :formats="['hex']" :show-alpha="false" show-actions @update:model-value="previewColor" @save="saveColor" @cancel="setColorOpen(lane, false)" />
                  </PopoverContent>
                </Popover>
                <Button variant="outline" size="sm" class="font-normal" :disabled="!value.waveChannelColors.some(Boolean)" @click="emit('change', { waveChannelColors: [null, null] })">{{ t('settings.waveform.autoColors') }}</Button>
              </div>
            </div>
            <template v-if="!opacityDisabled()">
              <div v-for="key in opacityControls" :key="key" class="option-row">
                <label :id="`${id}-${key}`" class="text-sm">{{ t(`settings.waveform.${key}`) }}</label>
                <div class="option-control flex items-center gap-3">
                  <Slider
                    :model-value="[Math.round(value[key] * 100)]" :min="0" :max="100" :step="5"
                    :aria-label="t(`settings.waveform.${key}`)"
                    @update:model-value="numericOption(key, $event)" @value-commit="numericOption(key, $event, true)"
                  />
                  <output class="w-12 shrink-0 text-right text-xs tabular-nums text-muted-foreground">{{ Math.round(value[key] * 100) }}%</output>
                </div>
              </div>
            </template>
          </template>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.wave-controls { container-type: inline-size; }
.option-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-height: 56px;
  padding: 12px 0;
}
.option-control { flex: 0 0 288px; width: 288px; min-width: 0; }
.dependent-row { min-height: 44px; padding-top: 8px; padding-bottom: 8px; }
@container (max-width: 600px) {
  .option-row { flex-direction: column; align-items: stretch; gap: 10px; }
  .option-control { flex-basis: auto; width: 100%; }
}
</style>
