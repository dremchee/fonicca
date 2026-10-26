<script setup lang="ts">
import type { MeterStyle } from '@/api'
import { useI18n } from 'vue-i18n'
import LevelMeter from '@/components/LevelMeter.vue'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const store = useSettingsStore()
const variants: MeterStyle[] = ['bars', 'segments', 'vertical', 'compact']
</script>

<template>
  <section class="flex flex-col gap-2" :aria-label="t('settings.playback.meter')">
    <h3 class="text-xs font-semibold">{{ t('settings.playback.meter') }}</h3>
    <p class="text-xs text-muted-foreground">{{ t('settings.playback.meterDesc') }}</p>
    <div class="grid grid-cols-2 gap-2" role="group" :aria-label="t('settings.playback.meter')">
      <button
        v-for="variant in variants" :key="variant" type="button"
        class="flex min-w-0 items-center gap-3 rounded-md border px-3 py-2 text-left text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :class="(store.settings.meterStyle ?? 'bars') === variant ? 'border-primary bg-primary-soft' : 'border-border hover:bg-accent'"
        :aria-pressed="(store.settings.meterStyle ?? 'bars') === variant" @click="store.update({ meterStyle: variant })"
      >
        <span class="flex h-9 w-24 shrink-0 items-center justify-center">
          <LevelMeter horizontal preview :variant="variant" :active="false" />
        </span>
        <span class="min-w-0">{{ t(`settings.playback.meterStyles.${variant}`) }}</span>
      </button>
    </div>
  </section>
</template>
