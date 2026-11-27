<script setup lang="ts">
import type { TrackTags } from '@/api'
import { Info, Music2, Pause, Play, SkipBack, SkipForward, Square } from '@lucide/vue'
import { useObjectUrl } from '@vueuse/core'
import { computed, shallowRef, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import Hint from '@/components/Hint.vue'
import LibraryPlayerVolume from '@/components/LibraryPlayerVolume.vue'
import PlaybackProgress from '@/components/PlaybackProgress.vue'
import PlaybackSpectrum from '@/components/PlaybackSpectrum.vue'
import SampleInfoHover from '@/components/SampleInfoHover.vue'
import { Button } from '@/components/ui/button'
import { useMiniPlayerPosition } from '@/composables/miniPlayerPosition'
import { useMiniPlayerTransport } from '@/composables/miniPlayerTransport'
import { trackInfoRevision } from '@/composables/trackInfo'
import { usePlayerStore } from '@/stores/player'

const { t } = useI18n()
const player = usePlayerStore()
const { sample, state, busy, canPrevious, canNext, toggle, stop, move } = useMiniPlayerTransport()
const panel = useTemplateRef<HTMLElement>('panel')
const { isDragging, style: positionStyle, moveWithKeys } = useMiniPlayerPosition(panel)
const cover = shallowRef<Blob>()
const tags = shallowRef<TrackTags | null>(null)
const coverUrl = useObjectUrl(cover)
const lamp = shallowRef({ color: 'var(--primary)', intensity: 0 })
const active = computed(() => state.value === 'playing')
const action = computed(() => t(active.value ? 'palette.pause' : 'palette.play', { name: sample.value?.name ?? '' }))
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

</script>

<template>
  <div
    v-if="sample" ref="panel" data-mini-player class="mini-player flex items-center gap-2.5 min-w-0"
    :class="{ 'is-dragging': isDragging }" :style="positionStyle" role="group" tabindex="0"
    :aria-label="t('player.miniPlayer')" :aria-description="t('player.moveHint')" @keydown.self="moveWithKeys"
  >
    <div class="player-lamp" :class="{ 'lamp-idle': !active }" :style="{ color: lamp.color, opacity: lamp.intensity ** 1.2 * 0.42 }" aria-hidden="true" />
    <div class="now-playing-cover size-10 shrink-0 rounded overflow-hidden bg-muted flex items-center justify-center" aria-hidden="true">
      <img v-if="coverUrl" :src="coverUrl" alt="" class="size-full object-cover" @error="cover = undefined">
      <Music2 v-else class="size-4 text-muted-foreground" />
    </div>
    <div class="min-w-0 flex-1">
      <div class="now-playing-heading flex items-baseline gap-3 min-w-0">
        <div class="min-w-0 flex-1 flex items-center gap-1">
          <div class="min-w-0 text-sm font-medium truncate" :title="sample.name">{{ sample.name }}</div>
          <SampleInfoHover :sample="sample" :tags="tags" :cover-url="coverUrl ?? null">
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
      <PlaybackSpectrum :key="sample.id" :state="state" class="mt-1" @glow="lamp = $event" />
    </div>
    <div data-player-control class="flex shrink-0 items-center gap-0.5">
      <Hint :text="t('player.previousTrack')" side="top">
        <Button variant="ghost" size="icon-sm" :aria-label="t('player.previousTrack')" :aria-disabled="busy || undefined" :disabled="!canPrevious" @click="move(-1)">
          <SkipBack />
        </Button>
      </Hint>
      <Hint :text="action" side="top">
        <Button variant="ghost" size="icon-sm" class="playback-toggle" :aria-label="action" :aria-disabled="busy || undefined" @click="toggle">
          <component :is="active ? Pause : Play" class="fill-current" />
        </Button>
      </Hint>
      <Hint :text="t('player.stop')" side="top">
        <Button variant="ghost" size="icon-sm" :aria-label="t('player.stop')" :aria-disabled="busy || undefined" :disabled="state === 'stopped'" @click="stop">
          <Square class="size-3 fill-current" />
        </Button>
      </Hint>
      <Hint :text="t('player.nextTrack')" side="top">
        <Button variant="ghost" size="icon-sm" :aria-label="t('player.nextTrack')" :aria-disabled="busy || undefined" :disabled="!canNext" @click="move(1)">
          <SkipForward />
        </Button>
      </Hint>
    </div>
    <LibraryPlayerVolume data-player-control />
    <PlaybackProgress :key="sample.id" data-player-control :position="player.position" :duration="sample.duration"
      :stopped="state === 'stopped'" @seek="player.seek(sample, $event)" @toggle="toggle" />
  </div>
</template>

<style scoped>
.mini-player {
  --panel-width: min(600px, calc(100% - 24px));
  --panel-height: min(56px, 100%);
  position: absolute;
  left: clamp(0px, var(--mini-player-x, calc((100% - var(--panel-width)) / 2)), calc(100% - var(--panel-width)));
  top: clamp(0px, var(--mini-player-y, calc(100% - var(--panel-height) - 12px)), calc(100% - var(--panel-height)));
  z-index: 40;
  width: var(--panel-width);
  height: var(--panel-height);
  padding: 6px 12px 8px 8px;
  container-type: inline-size;
  isolation: isolate;
  overflow: hidden;
  border-radius: 12px;
  background: color-mix(in srgb, var(--background) 94%, var(--foreground));
  box-shadow: 0 8px 28px rgb(0 0 0 / 30%), 0 0 0 1px color-mix(in srgb, var(--foreground) 9%, transparent);
  cursor: grab;
  touch-action: none;
  user-select: none;
}
.mini-player.is-dragging { cursor: grabbing; }
.mini-player :is(button, [data-player-control]) { cursor: default; }
.mini-player:focus-visible { outline: 1px solid var(--primary); }
@supports ((-webkit-backdrop-filter: blur(1px)) or (backdrop-filter: blur(1px))) {
  .mini-player {
    background: color-mix(in srgb, var(--background) 46%, transparent);
    -webkit-backdrop-filter: blur(5px) saturate(120%);
    backdrop-filter: blur(5px) saturate(120%);
  }
}
@media (prefers-reduced-transparency: reduce) {
  .mini-player {
    background: color-mix(in srgb, var(--background) 94%, var(--foreground));
    -webkit-backdrop-filter: none;
    backdrop-filter: none;
  }
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
@media (prefers-reduced-motion: no-preference) {
  .lamp-idle { transition: opacity 220ms ease-out; }
}
.playback-toggle, .playback-toggle:hover { color: var(--primary); }
.now-playing-time { color: color-mix(in srgb, var(--foreground) 80%, var(--background)); }
@container (max-width: 420px) {
  .now-playing-cover { display: none; }
}
@container (max-width: 365px) {
  .now-playing-time { display: none; }
}
</style>
