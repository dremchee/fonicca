<script setup lang="ts">
import { onBeforeUnmount, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import { invoke } from '@tauri-apps/api/core'

// A peak meter like a DAW's master: per channel, RMS solid and peak translucent
// above it, a peak-hold line, and a clip light that stays on until clicked.
const props = defineProps<{
  /** Something is playing: poll the levels. When it stops, the bars fall back. */
  active: boolean
  project?: boolean
  horizontal?: boolean
}>()

const { t } = useI18n()

const FLOOR = -70
/** dB per second the bars fall when the level drops. */
const FALL = 26
/** Peak hold: how long it stays, then how fast it falls. */
const HOLD_MS = 1200
const HOLD_FALL = 14

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

interface Channel { peak: number, rms: number, hold: number, holdAt: number }
const channels = reactive<Channel[]>([0, 1].map(() => ({ peak: FLOOR, rms: FLOOR, hold: FLOOR, holdAt: 0 })))
const clipped = ref(false)

const ticks = [0, -3, -6, -9, -12, -18, -24, -36, -48]
// Colour zones on the bar: the accent, amber from -6 dB, red from -1 dB.
const barGradient = `linear-gradient(to top, var(--primary) 0%, var(--primary) ${pct(-6)}, #e5a93a ${pct(-6)}, #e5a93a ${pct(-1)}, var(--destructive) ${pct(-1)})`

let frame = 0
let last = performance.now()
let pending = false
let disposed = false

async function tick(now: number) {
  const dt = Math.min(0.1, (now - last) / 1000)
  last = now
  let next: { peak: number[], rms: number[] } | null = null
  if (props.active && !pending) {
    pending = true
    try {
      next = props.project ? await invoke<Awaited<ReturnType<typeof api.playbackLevels>>>('project_levels') : await api.playbackLevels()
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
    if (peakLin >= 0.999)
      clipped.value = true
    const peak = toDb(peakLin)
    const rms = toDb(rmsLin)
    ch.peak = Math.max(peak, ch.peak - FALL * dt)
    ch.rms = Math.max(rms, ch.rms - FALL * dt)
    if (peak >= ch.hold) {
      ch.hold = peak
      ch.holdAt = now
    }
    else if (now - ch.holdAt > HOLD_MS) {
      ch.hold = Math.max(FLOOR, ch.hold - HOLD_FALL * dt)
    }
    moving ||= ch.peak > FLOOR || ch.hold > FLOOR
  })
  // Keep animating while playing or while the bars are still falling back.
  frame = props.active || moving ? requestAnimationFrame(tick) : 0
}

function start() {
  if (!frame) {
    last = performance.now()
    frame = requestAnimationFrame(tick)
  }
}

watch(() => props.active, (on) => {
  if (on)
    start()
}, { immediate: true })
onBeforeUnmount(() => {
  disposed = true
  cancelAnimationFrame(frame)
})
</script>

<template>
  <button
    v-if="horizontal" class="horizontal-meter" :class="{ clipped }"
    :aria-label="`${t('player.meter.title')}. ${t(clipped ? 'player.meter.clip' : 'player.meter.noClip')}`"
    :aria-pressed="clipped" :title="`${t('player.meter.title')} · ${t(clipped ? 'player.meter.clip' : 'player.meter.noClip')}`"
    @click="clipped = false"
  >
    <span v-for="(ch, i) in channels" :key="i" class="horizontal-lane" aria-hidden="true">
      <span class="horizontal-peak" :style="{ width: pct(ch.peak) }" />
      <span class="horizontal-rms" :style="{ width: pct(ch.rms) }" />
      <span v-if="ch.hold > FLOOR" class="horizontal-hold" :style="{ left: `min(${pct(ch.hold)}, calc(100% - 1px))` }" />
    </span>
    <span class="horizontal-clip" aria-hidden="true" />
  </button>
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
      <div v-for="(ch, i) in channels" :key="i" class="bar" :style="{ backgroundImage: barGradient }" aria-hidden="true">
        <!-- Covers from the top: the unlit part, then the peak part dimmed above the RMS. -->
        <div class="cover" :style="{ height: `calc(100% - ${pct(ch.peak)})` }" />
        <div class="cover dim" :style="{ bottom: pct(ch.rms), height: `max(0%, calc(${pct(ch.peak)} - ${pct(ch.rms)}))` }" />
        <div v-for="db in ticks" :key="db" class="tick" :style="{ bottom: pct(db) }" />
        <div v-if="ch.hold > FLOOR" class="hold" :style="{ bottom: pct(ch.hold) }" />
      </div>
    </div>

  </div>
</template>

<style scoped>
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
  inset-block: 0;
  left: 0;
  background: var(--foreground);
}
.horizontal-peak { opacity: 0.25; }
.horizontal-rms { opacity: 0.8; }
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
