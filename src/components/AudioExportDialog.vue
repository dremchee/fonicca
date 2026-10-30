<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { AUDIO_EXPORT_FORMATS, AUDIO_EXPORT_SAMPLE_RATES, MP3_SAMPLE_RATES, supportedExportRate, type AudioExportFormat } from '@/audio-export'
import { formatNumber } from '@/format'
import { pendingAudioExport, settleAudioExport } from '@/composables/audioExport'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const { t } = useI18n()
const format = ref<AudioExportFormat>('wav')
const rateChoice = ref('source')
const requestedRate = computed(() => rateChoice.value === 'source' ? null : Number(rateChoice.value))
const sourceRate = ref(44100)
const rates = computed(() => AUDIO_EXPORT_SAMPLE_RATES.filter(rate => format.value !== 'mp3' || MP3_SAMPLE_RATES.includes(rate)))
const sourceSupported = computed(() => format.value !== 'mp3' || MP3_SAMPLE_RATES.includes(sourceRate.value))
const bitrate = computed(() => (requestedRate.value ?? sourceRate.value) >= 32000 ? 320 : (requestedRate.value ?? sourceRate.value) >= 16000 ? 160 : 64)
watch(pendingAudioExport, value => {
  if (!value) return
  sourceRate.value = value.sourceRate
  format.value = value.format
  const rate = supportedExportRate(value.format, value.sampleRate, value.sourceRate)
  rateChoice.value = rate === null ? 'source' : String(rate)
})
watch(format, value => {
  const rate = supportedExportRate(value, requestedRate.value, sourceRate.value)
  rateChoice.value = rate === null ? 'source' : String(rate)
})
</script>

<template>
  <Dialog :open="!!pendingAudioExport" @update:open="!$event && settleAudioExport(null)">
    <DialogContent class="sm:max-w-sm">
      <DialogHeader>
        <DialogTitle>{{ t('editor.save.exportTitle') }}</DialogTitle>
        <DialogDescription>{{ t('editor.save.exportDescription') }}</DialogDescription>
      </DialogHeader>
      <form class="flex flex-col gap-4" @submit.prevent="settleAudioExport({ format, sampleRate: requestedRate })">
        <FieldGroup>
          <Field>
            <FieldLabel for="audio-export-format">{{ t('editor.save.format') }}</FieldLabel>
            <Select v-model="format">
              <SelectTrigger id="audio-export-format" class="w-full"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem v-for="value in AUDIO_EXPORT_FORMATS" :key="value" :value="value">{{ value === 'ogg' ? 'OGG Vorbis' : value.toUpperCase() }}</SelectItem>
              </SelectContent>
            </Select>
          </Field>
          <Field>
            <FieldLabel for="audio-export-rate">{{ t('editor.save.sampleRate') }}</FieldLabel>
            <Select v-model="rateChoice">
              <SelectTrigger id="audio-export-rate" class="w-full"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="source" :disabled="!sourceSupported">{{ t('editor.save.sourceRate', { rate: formatNumber(sourceRate / 1000) }) }}</SelectItem>
                <SelectItem v-for="rate in rates" :key="rate" :value="String(rate)">{{ t('editor.save.rateKhz', { rate: formatNumber(rate / 1000) }) }}</SelectItem>
              </SelectContent>
            </Select>
          </Field>
        </FieldGroup>
        <p class="text-sm text-muted-foreground">{{ t(`editor.save.formats.${format}`, { bitrate }) }}</p>
        <p v-if="format !== 'wav'" class="text-xs text-muted-foreground">{{ t('editor.save.wavMetadata') }}</p>
        <DialogFooter>
          <Button type="button" variant="ghost" @click="settleAudioExport(null)">{{ t('common.actions.cancel') }}</Button>
          <Button type="submit">{{ t('dialogs.continue') }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
