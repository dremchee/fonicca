<script setup lang="ts">
import type { MeterStyle } from '@/api'
import { Check } from '@lucide/vue'
import { shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import LevelMeter from '@/components/LevelMeter.vue'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const store = useSettingsStore()
const variants: MeterStyle[] = ['arcs', 'dots', 'spectrum', 'bars']
const demo = shallowRef<{ variant: MeterStyle, run: number } | null>(null)
function select(variant: MeterStyle) {
  demo.value = { variant, run: (demo.value?.run ?? 0) + 1 }
  void store.update({ meterStyle: variant })
}
</script>

<template>
  <section class="flex flex-col gap-2" :aria-label="t('settings.playback.meter')">
    <h3 class="text-xs font-semibold">{{ t('settings.playback.meter') }}</h3>
    <p class="text-xs text-muted-foreground">{{ t('settings.playback.meterDesc') }}</p>
    <div class="grid grid-cols-2 sm:grid-cols-4 gap-3" role="group" :aria-label="t('settings.playback.meter')">
      <button
        v-for="variant in variants" :key="variant" type="button"
        class="meter-card min-w-0 p-2 text-left text-xs"
        :class="{ selected: (store.settings.meterStyle ?? 'bars') === variant }"
        :aria-pressed="(store.settings.meterStyle ?? 'bars') === variant" @click="select(variant)"
      >
        <span class="flex h-[84px] w-full items-center justify-center rounded-[calc(var(--radius)-2px)] border border-border bg-background">
          <LevelMeter horizontal preview :variant="variant" :active="false" :preview-animation="demo?.variant === variant ? demo.run : 0" />
        </span>
        <span class="flex min-w-0 items-center gap-1.5 pt-2">
          <Check v-if="(store.settings.meterStyle ?? 'bars') === variant" class="selection-check size-3.5 text-primary shrink-0" aria-hidden="true" />
          <span class="font-medium truncate">{{ t(`settings.playback.meterStyles.${variant}`) }}</span>
        </span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.meter-card {
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--card);
  cursor: pointer;
  outline: none;
  transition: border-color 150ms, box-shadow 150ms;
}
.meter-card:hover { border-color: var(--muted-foreground); }
.meter-card.selected,
.meter-card:focus-visible {
  border-color: var(--primary);
  box-shadow: 0 0 0 1px var(--primary);
}
.selection-check { animation: select-check 160ms ease-out; }
@keyframes select-check {
  from { opacity: 0; transform: scale(0.7); }
  to { opacity: 1; transform: scale(1); }
}
@media (prefers-reduced-motion: reduce) {
  .meter-card { transition: none; }
  .selection-check { animation: none; }
}
</style>
