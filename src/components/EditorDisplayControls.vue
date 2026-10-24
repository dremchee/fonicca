<script setup lang="ts">
import { AudioWaveform, ChartColumnIncreasing, TriangleAlert } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { Spinner } from '@/components/ui/spinner'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'

const mode = defineModel<'waveform' | 'spectrogram'>({ required: true })
defineProps<{ loading: boolean, error: string | null }>()
const emit = defineEmits<{ retry: [] }>()
const { t } = useI18n()
// Hint also sets data-state, so the active view is styled by aria-pressed.
const iconClass = 'size-8 min-w-8 px-0 rounded-md aria-pressed:bg-transparent aria-pressed:text-primary aria-pressed:hover:bg-accent dark:aria-pressed:hover:bg-accent/50'
</script>

<template>
  <ToggleGroup
    :model-value="mode" type="single" size="sm" :spacing="1" class="shrink-0 gap-1"
    :aria-label="t('editor.spectrogram.display')"
    @update:model-value="($event === 'waveform' || $event === 'spectrogram') && (mode = $event)"
  >
    <Hint :text="t('editor.spectrogram.waveform')" side="top">
      <ToggleGroupItem value="waveform" :class="iconClass" :aria-label="t('editor.spectrogram.waveform')">
        <AudioWaveform />
      </ToggleGroupItem>
    </Hint>
    <Hint :text="error ? `${t('editor.spectrogram.error')} · ${t('editor.spectrogram.retry')}` : loading ? t('editor.spectrogram.loading') : t('editor.spectrogram.title')" side="top">
      <ToggleGroupItem
        value="spectrogram" :class="iconClass" :aria-label="t('editor.spectrogram.title')"
        :aria-busy="mode === 'spectrogram' && loading"
        @click="error && mode === 'spectrogram' && emit('retry')"
      >
        <Spinner v-if="mode === 'spectrogram' && loading" aria-hidden="true" />
        <TriangleAlert v-else-if="error" class="text-destructive" />
        <ChartColumnIncreasing v-else />
      </ToggleGroupItem>
    </Hint>
  </ToggleGroup>
  <span v-if="mode === 'spectrogram' && loading" role="status" class="sr-only">{{ t('editor.spectrogram.loading') }}</span>
  <span v-else-if="mode === 'spectrogram' && error" role="alert" class="sr-only">{{ t('editor.spectrogram.error') }}</span>
</template>
