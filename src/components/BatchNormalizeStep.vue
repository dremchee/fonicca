<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Field, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'

const enabled = defineModel<boolean>('enabled', { required: true })
const mode = defineModel<'peak' | 'lufs'>('mode', { required: true })
const peakDb = defineModel<number>('peakDb', { required: true })
const target = defineModel<number>('target', { required: true })
const ceiling = defineModel<number>('ceiling', { required: true })
const { t } = useI18n()
const within = (value: number, min: number, max: number) => Number.isFinite(value) && value >= min && value <= max
</script>

<template>
  <section class="p-3 flex flex-col gap-3">
    <Field orientation="horizontal">
      <FieldLabel for="batch-normalize" class="font-medium">{{ t('batch.normalize.title') }}</FieldLabel>
      <Switch id="batch-normalize" v-model="enabled" />
    </Field>
    <template v-if="enabled">
      <ToggleGroup :model-value="mode" type="single" variant="outline" size="sm" :aria-label="t('batch.normalize.mode')" @update:model-value="($event === 'peak' || $event === 'lufs') && (mode = $event)">
        <ToggleGroupItem value="peak">{{ t('batch.normalize.byPeak') }}</ToggleGroupItem>
        <ToggleGroupItem value="lufs">LUFS</ToggleGroupItem>
      </ToggleGroup>
      <Field v-if="mode === 'peak'" class="max-w-48 gap-1" :data-invalid="!within(peakDb, -96, 0)">
        <FieldLabel for="batch-peak">{{ t('batch.normalize.peak') }} (dBFS)</FieldLabel>
        <Input id="batch-peak" v-model.number="peakDb" :aria-invalid="!within(peakDb, -96, 0)" type="number" min="-96" max="0" step="0.1" class="h-8" />
      </Field>
      <FieldGroup v-else class="gap-2">
        <div class="grid grid-cols-2 gap-3">
          <Field class="gap-1" :data-invalid="!within(target, -36, -5)">
            <FieldLabel for="batch-lufs-target">{{ t('batch.normalize.target') }} (LUFS)</FieldLabel>
            <Input id="batch-lufs-target" v-model.number="target" :aria-invalid="!within(target, -36, -5)" type="number" min="-36" max="-5" step="0.1" class="h-8" />
          </Field>
          <Field class="gap-1" :data-invalid="!within(ceiling, -12, 0)">
            <FieldLabel for="batch-lufs-ceiling">{{ t('batch.normalize.ceiling') }} (dBFS)</FieldLabel>
            <Input id="batch-lufs-ceiling" v-model.number="ceiling" :aria-invalid="!within(ceiling, -12, 0)" type="number" min="-12" max="0" step="0.1" class="h-8" />
          </Field>
        </div>
        <FieldDescription>{{ t('batch.normalize.lufsHint') }}</FieldDescription>
      </FieldGroup>
    </template>
  </section>
</template>
