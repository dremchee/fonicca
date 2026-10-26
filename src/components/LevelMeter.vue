<script setup lang="ts">
import type { MeterStyle, PlaybackLevels } from '@/api'
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import SpectrumDisplay from '@/components/SpectrumDisplay.vue'
import StereoLevelDisplay from '@/components/StereoLevelDisplay.vue'

// A peak meter like a DAW's master: per channel, RMS solid and peak translucent
// above it, a fading peak-hold line, and an automatically clearing clip light.
const props = defineProps<{
  /** Something is playing: poll the levels. When it stops, the bars fall back. */
  active: boolean
  horizontal?: boolean
  variant?: MeterStyle
  /** Static settings swatch; never polls playback. */
  preview?: boolean
  /** Increment to replay the settings demo without polling audio. */
  previewAnimation?: number
}>()

const { t } = useI18n()

const FLOOR = -70
/** dB per second the bars fall when the level drops. */
const FALL = 26
/** Peak markers hold briefly, then fade; clipping clears after the same interval. */
const HOLD_MS = 1200
const HOLD_FADE_MS = 600
const CLIP_HOLD_MS = 1800

/**
 * Meter deflection 0..1 for a level in dBFS, after IEC 60268-18 (digital peak
 * meters): the loud end gets most of the height, the quiet end is squeezed.
 */
function iec(db: number) {
  let d = 0
  if (db < -70)
    d = 0
  else if (db < -60)
    d = (db + 70) * 0.25
  else if (db < -50)
    d = (db + 60) * 0.5 + 2.5
  else if (db < -40)
    d = (db + 50) * 0.75 + 7.5
  else if (db < -30)
    d = (db + 40) * 1.5 + 15
  else if (db < -20)
    d = (db + 30) * 2 + 30
  else if (db < 0)
    d = (db + 20) * 2.5 + 50
  else
    d = 100
  return d / 100
}

const toDb = (lin: number) => lin > 0 ? Math.max(FLOOR, 20 * Math.log10(lin)) : FLOOR
const pct = (db: number) => `${iec(db) * 100}%`

interface Channel { peak: number, rms: number, hold: number, holdAt: number, holdOpacity: number }
const channels = reactive<Channel[]>([0, 1].map(() => ({ peak: FLOOR, rms: FLOOR, hold: FLOOR, holdAt: 0, holdOpacity: 0 })))
const previewChannels: Channel[] = [
  { peak: -3, rms: -12, hold: -1, holdAt: 0, holdOpacity: 1 },
  { peak: -8, rms: -17, hold: -5, holdAt: 0, holdOpacity: 1 },
]
const previewPhase = ref<number | null>(null)
let previewFrame = 0
watch(() => props.previewAnimation, (run) => {
  cancelAnimationFrame(previewFrame)
  previewPhase.value = null
  if (!props.preview || !run || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
  const started = performance.now()
  const animate = (now: number) => {
    const phase = Math.min(1, (now - started) / 1300)
    previewPhase.value = phase === 1 ? null : phase
    previewFrame = phase < 1 ? requestAnimationFrame(animate) : 0
  }
  previewFrame = requestAnimationFrame(animate)
})
const meterChannels = computed(() => {
  if (!props.preview) return channels
  if (previewPhase.value == null) return previewChannels
  const phase = previewPhase.value
  const envelope = Math.sin(Math.PI * phase) ** 2
  return previewChannels.map((channel, i) => {
    const motion = envelope * (9 * Math.sin(phase * Math.PI * 4 + i) - 3)
    return { ...channel, rms: Math.min(-3, channel.rms + motion), peak: Math.min(-0.5, channel.peak + motion), hold: Math.min(-0.5, channel.hold + motion * 0.4) }
  })
})
const shapeChannels = computed(() => meterChannels.value.map(ch => ({
  level: iec(ch.rms), peak: iec(ch.peak), hold: ch.holdOpacity > 0 ? iec(ch.hold) : 0,
})))
const spectrum = reactive(Array.from({ length: 16 }, () => FLOOR))
const previewSpectrum = [0.22, 0.36, 0.51, 0.65, 0.81, 0.72, 0.59, 0.67, 0.49, 0.54, 0.4, 0.32, 0.39, 0.25, 0.18, 0.1]
const spectrumLevels = computed(() => {
  if (!props.preview) return spectrum.map(db => Math.min(1, Math.max(0, (db + 60) / 60)))
  if (previewPhase.value == null) return previewSpectrum
  const phase = previewPhase.value
  const envelope = Math.sin(Math.PI * phase) ** 2
  return previewSpectrum.map((level, i) => Math.min(1, Math.max(0.04, level + envelope * (0.27 * Math.sin(i * 0.7 + phase * Math.PI * 5) - 0.05))))
})
const variant = computed(() => props.variant ?? 'bars')
const vertical = computed(() => variant.value === 'vertical')
const levelClip = (db: number) => vertical.value
  ? { clipPath: `inset(calc(100% - ${pct(db)}) 0 0 0)` }
  : { clipPath: `inset(0 calc(100% - ${pct(db)}) 0 0)` }
const holdStyle = (ch: Channel) => ({
  ...(vertical.value ? { bottom: `min(${pct(ch.hold)}, calc(100% - 1px))` } : { left: `min(${pct(ch.hold)}, calc(100% - 1px))` }),
  opacity: ch.holdOpacity, background: meterColor(ch.hold),
})
const clipped = ref(false)
let clippedAt = 0

const ticks = [0, -3, -6, -9, -12, -18, -24, -36, -48]
// Fixed dB zones: green, yellow from -6 dB, red from -1 dB.
const meterColor = (db: number) => db >= -1 ? '#ef4444' : db >= -6 ? '#eab308' : '#22c55e'
const gradient = (direction: string) => `linear-gradient(${direction}, #22c55e 0%, #22c55e ${pct(-6)}, #eab308 ${pct(-6)}, #eab308 ${pct(-1)}, #ef4444 ${pct(-1)})`
const barGradient = gradient('to top')
const horizontalGradient = gradient('to right')

let frame = 0
let last = performance.now()
let pending = false
let disposed = false

async function tick(now: number) {
  const dt = Math.min(0.1, (now - last) / 1000)
  last = now
  let next: PlaybackLevels | null = null
  if (props.active && !props.preview && !pending) {
    pending = true
    try {
      next = await api.playbackLevels(props.variant === 'spectrum')
    }
    catch {
      next = null
    }
    finally {
      pending = false
    }
  }
  if (disposed)
    return
  let moving = false
  channels.forEach((ch, i) => {
    // Mono feeds both bars.
    const peakLin = next ? next.peak[Math.min(i, next.peak.length - 1)] ?? 0 : 0
    const rmsLin = next ? next.rms[Math.min(i, next.rms.length - 1)] ?? 0 : 0
    if (peakLin >= 0.999) {
      clipped.value = true
      clippedAt = now
    }
    const peak = toDb(peakLin)
    const rms = toDb(rmsLin)
    ch.peak = Math.max(peak, ch.peak - FALL * dt)
    ch.rms = Math.max(rms, ch.rms - FALL * dt)
    if (peak >= ch.hold) {
      ch.hold = peak
      ch.holdAt = now
    }
    ch.holdOpacity = Math.max(0, 1 - Math.max(0, now - ch.holdAt - HOLD_MS) / HOLD_FADE_MS)
    if (ch.holdOpacity === 0)
      ch.hold = FLOOR
    moving ||= ch.peak > FLOOR || (ch.hold > FLOOR && ch.holdOpacity > 0)
  })
  spectrum.forEach((db, index) => {
    spectrum[index] = Math.max(toDb(next?.spectrum?.[index] ?? 0), db - FALL * dt)
    moving ||= spectrum[index] > FLOOR
  })
  if (clipped.value && now - clippedAt >= CLIP_HOLD_MS)
    clipped.value = false
  // Keep animating until the bars and held indicators have cleared.
  frame = props.active || moving || clipped.value ? requestAnimationFrame(tick) : 0
}

function start() {
  if (!frame) {
    last = performance.now()
    frame = requestAnimationFrame(tick)
  }
}

watch(() => props.active, (on) => {
  if (on && !props.preview)
    start()
}, { immediate: true })
onBeforeUnmount(() => {
  disposed = true
  cancelAnimationFrame(frame)
  cancelAnimationFrame(previewFrame)
})
</script>

<template>
  <component
    :is="preview ? 'span' : 'button'" v-if="horizontal && (variant === 'spectrum' || variant === 'arcs' || variant === 'dots')"
    class="vu-control" :aria-hidden="preview ? true : undefined"
    :aria-label="`${t('player.meter.title')}. ${t(clipped ? 'player.meter.clip' : 'player.meter.noClip')}`"
    :title="t('player.meter.title')" :aria-pressed="preview ? undefined : clipped"
    @click="clipped = false"
  >
    <SpectrumDisplay v-if="variant === 'spectrum'" :levels="spectrumLevels" :clipped="clipped && !preview" />
    <StereoLevelDisplay v-else :variant="variant as 'arcs' | 'dots'" :channels="shapeChannels" :clipped="clipped && !preview" />
  </component>
  <component
    :is="preview ? 'span' : 'button'"
    v-else-if="horizontal" class="horizontal-meter" :class="[{ clipped: clipped && !preview }, `meter-${variant}`]" :style="{ '--meter-gradient': vertical ? barGradient : horizontalGradient }"
    :aria-hidden="preview ? true : undefined"
    :aria-label="`${t('player.meter.title')}. ${t(clipped ? 'player.meter.clip' : 'player.meter.noClip')}`"
    :aria-pressed="clipped" :title="`${t('player.meter.title')} · ${t(clipped ? 'player.meter.clip' : 'player.meter.noClip')}`"
    @click="clipped = false"
  >
    <span v-for="(ch, i) in meterChannels" :key="i" class="horizontal-lane" aria-hidden="true">
      <span class="horizontal-peak" :style="levelClip(ch.peak)" />
      <span class="horizontal-rms" :style="levelClip(ch.rms)" />
      <span v-if="ch.hold > FLOOR" class="horizontal-hold" :style="holdStyle(ch)" />
    </span>
    <span class="horizontal-clip" aria-hidden="true" />
  </component>
  <div v-else
    class="level-meter w-[52px] shrink-0 flex flex-col gap-1 border-l border-border bg-card px-1.5 py-2 select-none"
    role="group" :aria-label="t('player.meter.title')" :title="t('player.meter.title')"
  >
    <div class="flex gap-[3px]">
      <span class="w-3.5 shrink-0" />
      <button
        class="h-1.5 w-[25px] rounded-sm transition-colors"
        :class="clipped ? 'bg-destructive' : 'bg-muted'"
        :aria-label="t(clipped ? 'player.meter.clip' : 'player.meter.noClip')"
        :aria-pressed="clipped"
        :title="t(clipped ? 'player.meter.clip' : 'player.meter.noClip')"
        @click="clipped = false"
      />
    </div>
    <div class="relative flex-1 min-h-0 flex gap-[3px]">
      <!-- Scale: dB below full scale, as DAW meters print it. -->
      <div class="relative w-3.5 shrink-0 text-[9px] leading-none text-muted-foreground tabular" aria-hidden="true">
        <span v-for="db in ticks" :key="db" class="absolute right-0 translate-y-1/2" :style="{ bottom: pct(db) }">{{ -db }}</span>
      </div>
      <div v-for="(ch, i) in meterChannels" :key="i" class="bar" :style="{ backgroundImage: barGradient }" aria-hidden="true">
        <!-- Covers from the top: the unlit part, then the peak part dimmed above the RMS. -->
        <div class="cover" :style="{ height: `calc(100% - ${pct(ch.peak)})` }" />
        <div class="cover dim" :style="{ bottom: pct(ch.rms), height: `max(0%, calc(${pct(ch.peak)} - ${pct(ch.rms)}))` }" />
        <div v-for="db in ticks" :key="db" class="tick" :style="{ bottom: pct(db) }" />
        <div v-if="ch.hold > FLOOR" class="hold" :style="{ bottom: pct(ch.hold), opacity: ch.holdOpacity, background: meterColor(ch.hold) }" />
      </div>
    </div>

  </div>
</template>

<style scoped>
.vu-control { display: block; padding: 0; border: 0; background: transparent; flex-shrink: 0; border-radius: 5px; }
.vu-control:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
.horizontal-meter {
  width: 96px;
  height: 26px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 3px;
  grid-template-rows: repeat(2, minmax(0, 1fr));
  gap: 2px;
  padding: 4px;
  border: 0;
  background: transparent;
  flex-shrink: 0;
}
.horizontal-meter:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.horizontal-lane {
  position: relative;
  overflow: hidden;
}
.horizontal-lane:nth-child(2) {
  grid-column: 1;
  grid-row: 2;
}
.horizontal-peak,
.horizontal-rms {
  position: absolute;
  inset: 0;
  background: var(--meter-gradient);
}
.horizontal-peak { opacity: 0.35; }
.horizontal-rms { opacity: 1; }
.horizontal-hold {
  position: absolute;
  inset-block: 0;
  width: 1px;
  background: var(--foreground);
  opacity: 0.75;
}
.horizontal-clip {
  grid-column: 2;
  grid-row: 1 / 3;
  background: transparent;
}
.clipped .horizontal-clip { background: var(--destructive); }

.meter-segments .horizontal-peak,
.meter-segments .horizontal-rms {
  mask-image: repeating-linear-gradient(to right, #000 0 4px, transparent 4px 6px);
}
.meter-compact {
  width: 56px;
  height: 20px;
}
.meter-vertical {
  width: 36px;
  height: 30px;
  grid-template-columns: repeat(2, minmax(0, 1fr)) 3px;
  grid-template-rows: 1fr;
}
.meter-vertical .horizontal-lane:nth-child(2) {
  grid-column: 2;
  grid-row: 1;
}
.meter-vertical .horizontal-hold {
  inset-block: auto;
  inset-inline: 0;
  width: auto;
  height: 1px;
}
.meter-vertical .horizontal-clip {
  grid-column: 3;
  grid-row: 1;
}

.bar {
  position: relative;
  width: 11px;
  border-radius: 2px;
  overflow: hidden;
}

.cover {
  position: absolute;
  left: 0;
  right: 0;
  top: 0;
  background: var(--muted);
}

.cover.dim {
  top: auto;
  opacity: 0.55;
}

.tick {
  position: absolute;
  left: 0;
  right: 0;
  height: 1px;
  background: var(--card);
  opacity: 0.5;
}

.hold {
  position: absolute;
  left: 0;
  right: 0;
  height: 2px;
  transform: translateY(50%);
  background: var(--foreground);
}
</style>
