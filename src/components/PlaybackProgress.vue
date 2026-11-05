<script setup lang="ts">
import { computed, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import { formatDuration } from '@/format'

const props = defineProps<{ position: number, duration: number | null }>()
const emit = defineEmits<{ seek: [seconds: number], toggle: [] }>()
const { t } = useI18n()
const hintId = useId()
const duration = computed(() => props.duration != null && Number.isFinite(props.duration) && props.duration > 0 ? props.duration : 0)
const position = computed(() => Number.isFinite(props.position) ? Math.max(0, Math.min(duration.value, props.position)) : 0)
const progress = computed(() => duration.value ? position.value / duration.value : 0)

function seek(event: Event) {
  emit('seek', Number((event.target as HTMLInputElement).value))
}

function onKeydown(event: KeyboardEvent) {
  const step = event.shiftKey ? 10 : 5
  let target: number
  switch (event.key) {
    case 'ArrowLeft': case 'ArrowDown': target = position.value - step; break
    case 'ArrowRight': case 'ArrowUp': target = position.value + step; break
    case 'Home': target = 0; break
    case 'End': target = duration.value; break
    case ' ':
      event.preventDefault()
      event.stopPropagation()
      emit('toggle')
      return
    default: return
  }
  event.preventDefault()
  event.stopPropagation()
  emit('seek', Math.max(0, Math.min(duration.value, target)))
}
</script>

<template>
  <div v-if="duration" class="playback-progress">
    <span class="progress-fill" :style="{ transform: `scaleX(${progress})` }" />
    <input
      type="range" class="progress-input" min="0" :max="duration" step="0.01" :value="position"
      :aria-label="t('player.seek')" :aria-describedby="hintId"
      :aria-valuetext="t('player.position', { elapsed: formatDuration(position), duration: formatDuration(duration) })"
      @input="seek" @keydown="onKeydown" @pointerdown.stop @mousedown.stop @click.stop @dblclick.stop
    >
    <span :id="hintId" class="sr-only">{{ t('player.seekHint') }}</span>
  </div>
</template>

<style scoped>
.playback-progress {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  z-index: 1;
  height: 2px;
  background: color-mix(in srgb, var(--primary) 15%, transparent);
}
.playback-progress:hover { height: 3px; }
.playback-progress:focus-within { height: 4px; }
.progress-fill {
  display: block;
  width: 100%;
  height: 100%;
  background: var(--primary);
  transform-origin: left;
  pointer-events: none;
}
.progress-input {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  width: 100%;
  height: 10px;
  margin: 0;
  padding: 0;
  appearance: none;
  opacity: 0;
  cursor: pointer;
}
.progress-input::-webkit-slider-thumb { appearance: none; width: 1px; height: 10px; }
.progress-input::-moz-range-thumb { width: 1px; height: 10px; border: 0; }
</style>
