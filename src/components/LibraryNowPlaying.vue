<script setup lang="ts">
import type { TrackTags } from '@/api'
import { Info, Music2, Pause, Play } from '@lucide/vue'
import { useObjectUrl } from '@vueuse/core'
import { computed, ref, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import Hint from '@/components/Hint.vue'
import PlaybackProgress from '@/components/PlaybackProgress.vue'
import PlaybackSpectrum from '@/components/PlaybackSpectrum.vue'
import SampleInfoHover from '@/components/SampleInfoHover.vue'
import { Button } from '@/components/ui/button'
import { trackInfoRevision } from '@/composables/trackInfo'
import { useEditorStore } from '@/stores/editor'
import { usePlayerStore } from '@/stores/player'

const { t } = useI18n()
const player = usePlayerStore()
const editor = useEditorStore()
const sample = computed(() => player.playingSample)
const cover = shallowRef<Blob>()
const tags = shallowRef<TrackTags | null>(null)
const coverUrl = useObjectUrl(cover)
const toggling = ref(false)
const lamp = shallowRef({ color: 'var(--primary)', intensity: 0 })
const action = computed(() => t(player.paused ? 'palette.play' : 'palette.pause', { name: sample.value?.name ?? '' }))
watch(() => sample.value?.id, () => { lamp.value = { color: 'var(--primary)', intensity: 0 } })

// Invalidate late reads when tracks change; useObjectUrl releases old images.
watch(() => [sample.value?.id, sample.value?.path, trackInfoRevision.value] as const, async ([id], _, onCleanup) => {
  cover.value = undefined
  tags.value = null
  let cancelled = false
  onCleanup(() => { cancelled = true })
  if (id == null) return
  try {
    const [image, metadata] = await Promise.allSettled([api.sampleCover(id), api.sampleTags(id)])
    if (cancelled) return
    if (image.status === 'fulfilled' && image.value?.byteLength) cover.value = new Blob([image.value])
    if (metadata.status === 'fulfilled') tags.value = metadata.value
  }
  catch { /* Untagged or unavailable files use the music icon. */ }
}, { immediate: true })

function clock(seconds: number | null) {
  if (seconds == null || !Number.isFinite(seconds)) return '—'
  const whole = Math.max(0, Math.floor(seconds))
  const hours = Math.floor(whole / 3600)
  const minutes = Math.floor(whole / 60) % 60
  const remainder = String(whole % 60).padStart(2, '0')
  return hours ? `${hours}:${String(minutes).padStart(2, '0')}:${remainder}` : `${minutes}:${remainder}`
}

async function toggle() {
  const current = sample.value
  if (!current || toggling.value) return
  toggling.value = true
  try {
    if (player.paused) await editor.stopPlayback()
    // A row action may have changed the audition while the editor stopped.
    if (sample.value?.id === current.id) await player.toggle(current)
  }
  finally { toggling.value = false }
}
</script>

<template>
  <div v-if="sample" class="now-playing flex items-center gap-2 min-w-0" role="group" :aria-label="t('player.nowPlaying')">
    <div class="player-lamp" :style="{ color: lamp.color, opacity: lamp.intensity ** 1.2 * 0.65 }" aria-hidden="true" />
    <div class="now-playing-cover size-10 shrink-0 rounded overflow-hidden bg-muted flex items-center justify-center" aria-hidden="true">
      <img v-if="coverUrl" :src="coverUrl" alt="" class="size-full object-cover" @error="cover = undefined">
      <Music2 v-else class="size-4 text-muted-foreground" />
    </div>
    <Hint :text="action">
      <Button type="button" variant="ghost" size="icon-sm" class="playback-toggle shrink-0" :aria-label="action" :disabled="toggling" @click="toggle">
        <component :is="player.paused ? Play : Pause" class="fill-current" />
      </Button>
    </Hint>
    <div class="min-w-0 flex-1">
      <div class="now-playing-heading flex items-baseline gap-3 min-w-0">
        <div class="min-w-0 flex-1 flex items-center gap-1">
          <div class="min-w-0 text-sm font-medium truncate" :title="sample.name">{{ sample.name }}</div>
          <SampleInfoHover :sample="sample" :tags="tags" :cover-url="coverUrl ?? null" side="bottom" :side-offset="30">
            <template #trigger>
              <Button type="button" variant="ghost" size="icon-xs" class="size-4 shrink-0 text-muted-foreground" :aria-label="t('trackInfo.title')" :title="t('trackInfo.title')">
                <Info class="size-3.5" />
              </Button>
            </template>
          </SampleInfoHover>
        </div>
        <span class="now-playing-time shrink-0 text-xs tabular whitespace-nowrap" :aria-label="t('player.position', { elapsed: clock(player.position), duration: clock(sample.duration) })">
          {{ clock(player.position) }} / {{ clock(sample.duration) }}
        </span>
      </div>
      <PlaybackSpectrum :key="sample.id" :active="!player.paused" class="mt-1" @glow="lamp = $event" />
    </div>
    <PlaybackProgress :key="sample.id" :position="player.position" :duration="sample.duration" @seek="player.seek(sample, $event)" @toggle="toggle" />
  </div>
</template>

<style scoped>
.now-playing {
  position: relative;
  isolation: isolate;
  overflow: hidden;
  justify-self: end;
  width: 100%;
  min-width: min(360px, 100%);
  max-width: min(504px, 100%);
  height: 44px;
  padding: 4px 6px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--background) 65%, #000 35%);
}
.player-lamp {
  position: absolute;
  inset: 0;
  overflow: clip;
  z-index: -1;
  pointer-events: none;
}
.player-lamp::before {
  content: '';
  position: absolute;
  inset: -12px -20px;
  background: radial-gradient(ellipse 48% 46% at 50% 70%, currentColor 0%, transparent 100%);
  filter: blur(12px);
}
.playback-toggle, .playback-toggle:hover { color: var(--primary); }
.now-playing-time { color: color-mix(in srgb, var(--foreground) 80%, var(--background)); }
@container (max-width: 620px) {
  .now-playing-cover { display: none; }
}
@container (max-width: 450px) {
  .now-playing-time { display: none; }
}
</style>
