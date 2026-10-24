<script setup lang="ts">
import type { ChannelDef, ColorFormat, HSVA } from './color'
import { Check, Copy, Pipette } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { cn } from 'cn'
import { useI18n } from 'vue-i18n'
import {
  clamp,
  COLOR_FORMATS,
  detectColorFormat,
  formatColor,
  getChannels,
  parseColor,
  parseHex,
  setChannel,
  toCssColor,
  toHex,
} from './color'
import ColorArea from './ColorArea.vue'
import ColorSlider from './ColorSlider.vue'

const props = withDefaults(defineProps<{
  /** Output/edit format. Acts as the initial value; the in-panel switcher can change it. */
  format?: ColorFormat
  /** Formats offered by the switcher; with one, the switcher is hidden. */
  formats?: readonly ColorFormat[]
  /** Show the alpha slider and channel. */
  showAlpha?: boolean
  /** Show the system eyedropper button (when the browser supports it). */
  showEyeDropper?: boolean
  /** Show a Save/Cancel footer (e.g. when hosted in a popover that commits on save). */
  showActions?: boolean
  class?: string
}>(), {
  format: 'hex',
  formats: () => COLOR_FORMATS,
  showAlpha: true,
  showEyeDropper: true,
  showActions: false,
})

const emit = defineEmits<{
  save: []
  cancel: []
}>()

const model = defineModel<string>({ default: '#000000' })
const { t } = useI18n()

// HSVA is the source of truth — see color.ts for why.
const hsva = ref<HSVA>(parseColor(model.value) ?? { h: 0, s: 0, v: 0, a: 1 })
// The incoming value's own notation wins over the format prop, so opening the
// picker on `oklch(…)` lands on the OKLCH tab instead of re-encoding to hex.
const detected = detectColorFormat(model.value)
const format = ref<ColorFormat>(detected && props.formats.includes(detected) ? detected : props.format)

// Track what we last pushed to the model so echoing it back doesn't re-parse
// (and reset) the HSVA state mid-interaction.
let lastEmitted = model.value

watch(model, (value) => {
  if (value === lastEmitted)
    return
  const parsed = parseColor(value ?? '')
  if (parsed) {
    hsva.value = parsed
    lastEmitted = value ?? ''
  }
})

watch(hsva, (value) => {
  const str = formatColor(value, format.value)
  lastEmitted = str
  if (str !== model.value)
    model.value = str
}, { deep: true })

watch(format, (next) => {
  const str = formatColor(hsva.value, next)
  lastEmitted = str
  model.value = str
})

// --- derived views ---------------------------------------------------------
const currentCss = computed(() => toCssColor(hsva.value))
const opaqueCss = computed(() => toCssColor({ ...hsva.value, a: 1 }))

const hue01 = computed({
  get: () => hsva.value.h / 360,
  set: n => (hsva.value = { ...hsva.value, h: clamp(n * 360, 0, 360) }),
})
const alpha = computed({
  get: () => hsva.value.a,
  set: n => (hsva.value = { ...hsva.value, a: clamp(n, 0, 1) }),
})

const hueTrackStyle = {
  backgroundImage:
    'linear-gradient(to right, #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%)',
}
const hueHandleStyle = computed(() => ({ backgroundColor: `hsl(${hsva.value.h}, 100%, 50%)` }))
const alphaTrackStyle = computed(() => ({
  backgroundImage: `linear-gradient(to right, rgba(0,0,0,0), ${opaqueCss.value})`,
}))
const alphaHandleStyle = computed(() => ({ backgroundColor: currentCss.value }))

// --- editing ---------------------------------------------------------------
function onArea({ s, v }: { s: number, v: number }) {
  hsva.value = { ...hsva.value, s, v }
}

// Hex gets a free-text draft so partially-typed values aren't reformatted mid-edit.
const hexDraft = ref('')
const hexFocused = ref(false)
const hexValue = computed(() =>
  hexFocused.value ? hexDraft.value : toHex(hsva.value, props.showAlpha),
)

function onHexFocus() {
  hexDraft.value = toHex(hsva.value, props.showAlpha)
  hexFocused.value = true
}
function onHexInput(event: Event) {
  hexDraft.value = (event.target as HTMLInputElement).value
  const parsed = parseHex(hexDraft.value)
  if (parsed)
    hsva.value = props.showAlpha ? parsed : { ...parsed, a: hsva.value.a }
}
function onHexBlur() {
  hexFocused.value = false
}

const channels = computed<ChannelDef[]>(() => getChannels(hsva.value, format.value))

function onChannelInput(channel: ChannelDef, event: Event) {
  const n = Number.parseFloat((event.target as HTMLInputElement).value)
  if (!Number.isNaN(n))
    hsva.value = setChannel(hsva.value, format.value, channel.key, clamp(n, channel.min, channel.max))
}

const alphaPercent = computed(() => Math.round(hsva.value.a * 100))
function onAlphaInput(event: Event) {
  const n = Number.parseFloat((event.target as HTMLInputElement).value)
  if (!Number.isNaN(n))
    alpha.value = clamp(n, 0, 100) / 100
}

// --- eyedropper ------------------------------------------------------------
interface EyeDropperResult { sRGBHex: string }
interface EyeDropperCtor { new (): { open: () => Promise<EyeDropperResult> } }
const EyeDropperImpl
  = typeof window !== 'undefined'
    ? (window as unknown as { EyeDropper?: EyeDropperCtor }).EyeDropper
    : undefined
const eyeDropperAvailable = computed(() => props.showEyeDropper && !!EyeDropperImpl)

async function pickWithEyeDropper() {
  if (!EyeDropperImpl)
    return
  try {
    const result = await new EyeDropperImpl().open()
    const parsed = parseColor(result.sRGBHex)
    if (parsed)
      hsva.value = { ...parsed, a: hsva.value.a }
  }
  catch {
    // User dismissed the picker — nothing to do.
  }
}

// --- copy ------------------------------------------------------------------
const copied = ref(false)
let copyTimer: ReturnType<typeof setTimeout> | undefined
async function copyValue() {
  try {
    await navigator.clipboard?.writeText(model.value)
    copied.value = true
    clearTimeout(copyTimer)
    copyTimer = setTimeout(() => (copied.value = false), 1200)
  }
  catch {
    // Clipboard unavailable — ignore.
  }
}

const channelInputClass
  = 'h-7 w-full min-w-0 rounded-sm border border-input bg-transparent px-1 text-center text-xs '
    + 'tabular-nums outline-none focus:border-ring focus:ring-1 focus:ring-ring'
</script>

<template>
  <div :class="cn('flex w-64 flex-col gap-3 text-foreground', props.class)">
    <ColorArea
      :hue="hsva.h"
      :saturation="hsva.s"
      :value="hsva.v"
      :handle-color="opaqueCss"
      @update="onArea"
    />

    <div class="flex items-center gap-2">
      <button
        v-if="eyeDropperAvailable"
        type="button"
        :aria-label="t('common.color.pickFromScreen')"
        class="grid size-8 shrink-0 place-items-center rounded-md border border-input text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        @click="pickWithEyeDropper"
      >
        <Pipette :size="15" />
      </button>

      <div class="flex min-w-0 flex-1 flex-col gap-2">
        <ColorSlider
          v-model="hue01"
          :label="t('common.color.hue')"
          :track-style="hueTrackStyle"
          :handle-style="hueHandleStyle"
        />
        <ColorSlider
          v-if="showAlpha"
          v-model="alpha"
          :label="t('common.color.alpha')"
          checkerboard
          :track-style="alphaTrackStyle"
          :handle-style="alphaHandleStyle"
        />
      </div>
    </div>

    <!-- Format switcher -->
    <ToggleGroup
      v-if="formats.length > 1"
      :model-value="format"
      type="single"
      variant="outline"
      size="sm"
      class="w-full"
      @update:model-value="(v) => v && (format = v as ColorFormat)"
    >
      <ToggleGroupItem
        v-for="f in formats"
        :key="f"
        :value="f"
        class="flex-1 px-1 text-[11px] uppercase leading-none"
      >
        {{ f }}
      </ToggleGroupItem>
    </ToggleGroup>

    <!-- Channel inputs -->
    <div class="flex items-end gap-1">
      <template v-if="format === 'hex'">
        <input
          :class="`${channelInputClass} px-2 text-left`"
          spellcheck="false"
          autocapitalize="off"
          autocomplete="off"
          :value="hexValue"
          :aria-label="t('common.color.hexValue')"
          @focus="onHexFocus"
          @input="onHexInput"
          @blur="onHexBlur"
        >
      </template>
      <template v-else>
        <label
          v-for="channel in channels"
          :key="channel.key"
          class="flex min-w-0 flex-1 flex-col items-center gap-0.5"
        >
          <input
            :class="channelInputClass"
            inputmode="decimal"
            :value="channel.value"
            :aria-label="channel.label"
            @input="(e) => onChannelInput(channel, e)"
          >
          <span class="text-[10px] text-muted-foreground">{{ channel.label }}</span>
        </label>
      </template>

      <label v-if="showAlpha" class="flex w-11 shrink-0 flex-col items-center gap-0.5">
        <input
          :class="channelInputClass"
          inputmode="numeric"
          :value="alphaPercent"
          :aria-label="t('common.color.alphaPercent')"
          @input="onAlphaInput"
        >
        <span v-if="format !== 'hex'" class="text-[10px] text-muted-foreground">A</span>
      </label>

      <button
        type="button"
        :aria-label="copied ? t('common.color.copied') : t('common.color.copy')"
        class="grid size-7 shrink-0 place-items-center self-start rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        @click="copyValue"
      >
        <Check v-if="copied" :size="14" class="text-emerald-500" />
        <Copy v-else :size="14" />
      </button>
    </div>

    <!-- Save / Cancel footer (opt-in) — matches the app's popover form footers. -->
    <div v-if="showActions" class="flex justify-end gap-1.5 pt-0.5">
      <Button type="button" variant="ghost" size="sm" @click="emit('cancel')">
        {{ t('common.actions.cancel') }}
      </Button>
      <Button type="button" size="sm" @click="emit('save')">
        {{ t('common.actions.save') }}
      </Button>
    </div>
  </div>
</template>
