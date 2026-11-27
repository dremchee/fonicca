<script setup lang="ts">
import { Volume2, VolumeX } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const settings = useSettingsStore()
const draft = shallowRef<number | null>(null)
const percent = computed(() => draft.value ?? Math.round(settings.settings.volume * 100))
const muted = computed(() => percent.value === 0)
const muteAction = computed(() => t(muted.value ? 'player.unmute' : 'player.mute'))
let audibleVolume = 100
let saving = Promise.resolve()

async function commit(values: number[]) {
  const value = values[0]
  if (value == null) return
  if (value > 0) audibleVolume = value
  draft.value = value
  const request = saving.then(() => settings.update({ volume: value / 100 }))
  saving = request
  await request
  if (saving === request && draft.value === value) draft.value = null
}

function toggleMute() {
  if (muted.value) return commit([audibleVolume])
  audibleVolume = percent.value
  return commit([0])
}
</script>

<template>
  <div class="player-volume flex items-center gap-2 shrink-0 text-muted-foreground">
    <Hint :text="muteAction" side="top">
      <Button type="button" variant="ghost" size="icon-xs" class="-mx-1 shrink-0" :aria-label="muteAction" @click="toggleMute">
        <component :is="muted ? VolumeX : Volume2" class="size-4" aria-hidden="true" />
      </Button>
    </Hint>
    <Slider
      :model-value="[percent]" :min="0" :max="100" :step="1" :aria-label="t('player.volume')"
      class="volume-slider h-7"
      @update:model-value="draft = $event?.[0] ?? null" @value-commit="commit" @pointercancel="draft = null"
    />
  </div>
</template>

<style scoped>
.player-volume { width: 5.75rem; }
.volume-slider :deep([data-slot='slider-track']) { height: 3px; }
.volume-slider :deep([data-slot='slider-thumb']) {
  width: 10px;
  height: 10px;
  border: 0;
  background: var(--primary);
}
@container (max-width: 420px) {
  .player-volume { width: 3.75rem; }
}
</style>
