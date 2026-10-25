<script setup lang="ts">
import type { WaveformSettings } from '@/api'
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import WaveformPreview from '@/components/WaveformPreview.vue'
import { Button } from '@/components/ui/button'
import { Field, FieldContent, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useSettingsStore } from '@/stores/settings'
import { waveformPreset, waveformSettings, WAVEFORM_PRESETS } from '@/waveform-settings'

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
const value = computed(() => ({ ...waveformSettings(props.options), ...draft.value }))
const preset = computed(() => waveformPreset(value.value))
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
const presets = ['editing', 'detail', 'music'] as const
const barControls = [
  { key: 'waveBarWidth', min: 1, max: 8, step: 0.5, scale: 1, unit: 'px' },
  { key: 'waveBarGap', min: 0, max: 6, step: 0.5, scale: 1, unit: 'px' },
  { key: 'waveBarRadius', min: 0, max: 100, step: 5, scale: 100, unit: '%' },
] as const
const opacityControls = ['wavePeakOpacity', 'waveRmsOpacity'] as const

function disabled(key: string) {
  return ((key === 'waveDensity' || key === 'waveThickness') && value.value.waveStyle === 'bars')
    || (key === 'waveDbRange' && value.value.waveScale !== 'db')
}
function option(key: typeof choices[number]['key'], next: unknown) {
  const row = choices.find(row => row.key === key)!
  const selected = row.items.find(item => String(item[0]) === next)
  if (selected)
    emit('change', { [key]: selected[0] })
}
function applyPreset(next: unknown) {
  if (presets.some(key => key === next))
    emit('change', waveformSettings(WAVEFORM_PRESETS[next as typeof presets[number]]))
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
function channelColor(lane: number, event: Event, commit = false) {
  const colors = [...value.value.waveChannelColors] as WaveformSettings['waveChannelColors']
  colors[lane] = (event.target as HTMLInputElement).value
  if (commit) {
    draft.value = {}
    emit('change', { waveChannelColors: colors })
  }
  else {
    draft.value = { waveChannelColors: colors }
  }
}
</script>

<template>
  <div class="flex flex-col gap-5">
    <div class="sticky top-0 bg-card pb-3 flex flex-col gap-3 wave-example">
      <div class="flex items-center justify-between gap-3">
        <span class="text-xs text-muted-foreground">{{ t('settings.waveform.preview') }}</span>
        <span v-if="preset === 'custom'" class="text-xs text-muted-foreground">{{ t('settings.waveform.custom') }}</span>
      </div>
      <WaveformPreview :options="value" :accent="props.target === 'editor'" />
      <ToggleGroup :model-value="preset" type="single" variant="outline" size="sm" :aria-label="t('settings.waveform.presets')" @update:model-value="applyPreset">
        <ToggleGroupItem v-for="key in presets" :key="key" :value="key" class="flex-1">
          {{ t(`settings.waveform.preset_${key}`) }}
        </ToggleGroupItem>
      </ToggleGroup>
    </div>

    <FieldGroup class="gap-5">
      <template v-for="row in choices" :key="row.key">
        <Field orientation="horizontal" :data-disabled="disabled(row.key)">
          <FieldContent class="min-w-0">
            <FieldLabel :id="`${id}-${row.key}`">{{ t(`settings.waveform.${row.label}`) }}</FieldLabel>
            <FieldDescription v-if="row.key === 'waveDensity'" class="text-xs">{{ t('settings.waveform.densityDesc') }}</FieldDescription>
          </FieldContent>
          <ToggleGroup
            :model-value="String(value[row.key])" type="single" variant="outline" size="sm"
            :disabled="disabled(row.key)" :aria-labelledby="`${id}-${row.key}`"
            @update:model-value="option(row.key, $event)"
          >
            <ToggleGroupItem v-for="item in row.items" :key="item[0]" :value="String(item[0])">
              {{ typeof item[0] === 'number' ? item[1] : t(`settings.waveform.${item[1]}`) }}
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>
        <FieldGroup v-if="row.key === 'waveStyle' && value.waveStyle === 'bars'" class="gap-4">
          <Field v-for="control in barControls" :key="control.key" orientation="horizontal">
            <FieldLabel class="flex-1">{{ t(`settings.waveform.${control.key}`) }}</FieldLabel>
            <div class="flex w-64 max-w-1/2 items-center gap-3">
              <Slider
                :model-value="[value[control.key] * control.scale]" :min="control.min" :max="control.max" :step="control.step"
                :aria-label="t(`settings.waveform.${control.key}`)"
                @update:model-value="numericOption(control.key, $event, false, control.scale)"
                @value-commit="numericOption(control.key, $event, true, control.scale)"
              />
              <output class="w-12 shrink-0 text-right text-xs tabular-nums text-muted-foreground">{{ Number((value[control.key] * control.scale).toFixed(1)) }} {{ control.unit }}</output>
            </div>
          </Field>
        </FieldGroup>
      </template>

      <Field orientation="horizontal" :data-disabled="value.waveColors === 'frequency'">
        <FieldContent>
          <FieldLabel :id="`${id}-channel-colors`">{{ t('settings.waveform.channelColors') }}</FieldLabel>
          <FieldDescription class="text-xs">{{ t('settings.waveform.channelColorsDesc') }}</FieldDescription>
        </FieldContent>
        <div class="flex shrink-0 items-center gap-3" role="group" :aria-labelledby="`${id}-channel-colors`">
          <label v-for="(_, lane) in value.waveChannels === 'split' ? 2 : 1" :key="lane" class="flex items-center gap-1.5 text-xs text-muted-foreground">
            {{ t(`settings.waveform.${value.waveChannels === 'mix' ? 'monoColor' : lane === 0 ? 'leftColor' : 'rightColor'}`) }}
            <input
              type="color" class="size-7 cursor-pointer rounded border border-border bg-transparent p-0.5 disabled:opacity-40 disabled:cursor-default"
              :aria-label="t(`settings.waveform.${value.waveChannels === 'mix' ? 'monoColor' : lane === 0 ? 'leftColor' : 'rightColor'}`)"
              :value="value.waveChannelColors[lane] ?? themeColor" :disabled="value.waveColors === 'frequency'"
              @input="channelColor(lane, $event)" @change="channelColor(lane, $event, true)"
            >
          </label>
          <Button variant="ghost" size="sm" :disabled="value.waveColors === 'frequency' || !value.waveChannelColors.some(Boolean)" @click="emit('change', { waveChannelColors: [null, null] })">
            {{ t('settings.waveform.autoColors') }}
          </Button>
        </div>
      </Field>

      <Field v-for="key in opacityControls" :key="key" orientation="horizontal" :data-disabled="opacityDisabled()">
        <FieldLabel :id="`${id}-${key}`" class="flex-1">{{ t(`settings.waveform.${key}`) }}</FieldLabel>
        <div class="flex w-64 max-w-1/2 items-center gap-3">
          <Slider
            :model-value="[Math.round(value[key] * 100)]" :min="0" :max="100" :step="5"
            :disabled="opacityDisabled()" :aria-label="t(`settings.waveform.${key}`)"
            @update:model-value="numericOption(key, $event)" @value-commit="numericOption(key, $event, true)"
          />
          <output class="w-10 shrink-0 text-right text-xs tabular-nums text-muted-foreground">{{ Math.round(value[key] * 100) }}%</output>
        </div>
      </Field>
    </FieldGroup>
  </div>
</template>

<style scoped>
.wave-example { z-index: 1; }
</style>
