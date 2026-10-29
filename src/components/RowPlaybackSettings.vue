<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ROW_PLAYBACK_MODES, type RowPlaybackMode } from '@/row-playback'

const props = defineProps<{ mode: RowPlaybackMode }>()
const emit = defineEmits<{ change: [mode: RowPlaybackMode] }>()
const { t } = useI18n()

function select(value: unknown) {
  if (ROW_PLAYBACK_MODES.includes(value as RowPlaybackMode)) emit('change', value as RowPlaybackMode)
}
</script>

<template>
  <section class="group-box p-4" :aria-label="t('settings.playback.rowClick.title')">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div class="min-w-0 flex-1 basis-48">
        <h3 class="text-sm font-medium">{{ t('settings.playback.rowClick.title') }}</h3>
        <p class="mt-1 text-xs leading-relaxed text-muted-foreground">{{ t(`settings.playback.rowClick.${props.mode}.description`) }}</p>
      </div>
      <Select :model-value="props.mode" @update:model-value="select">
        <SelectTrigger class="w-72 max-w-full" :aria-label="t('settings.playback.rowClick.title')">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="value in ROW_PLAYBACK_MODES" :key="value" :value="value">
            {{ t(`settings.playback.rowClick.${value}.title`) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  </section>
</template>
