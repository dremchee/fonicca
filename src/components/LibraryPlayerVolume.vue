<script setup lang="ts">
import { Volume2, VolumeX } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { Slider } from '@/components/ui/slider'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const settings = useSettingsStore()
const draft = shallowRef<number | null>(null)
const percent = computed(() => draft.value ?? Math.round(settings.settings.volume * 100))
let saving = Promise.resolve()

async function commit(values: number[]) {
  const value = values[0]
  if (value == null) return
  draft.value = value
  const request = saving.then(() => settings.update({ volume: value / 100 }))
  saving = request
  await request
  if (saving === request && draft.value === value) draft.value = null
}
</script>

<template>
  <div class="player-volume flex items-center gap-2 shrink-0 text-muted-foreground">
    <component :is="percent ? Volume2 : VolumeX" class="size-4 shrink-0" aria-hidden="true" />
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
