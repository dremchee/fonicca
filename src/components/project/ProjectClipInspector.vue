<script setup lang="ts">
import { Copy, Trash2 } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { useProjectStore } from '@/stores/project'
const project = useProjectStore()
const { t } = useI18n()
const fields = [
  { key: 'start', label: 'position', min: 0, max: 600, step: 0.01 },
  { key: 'offset', label: 'offset', min: 0, max: 600, step: 0.01 },
  { key: 'duration', label: 'length', min: 1 / 48000, max: 600, step: 0.01 },
  { key: 'repeats', label: 'repeats', min: 1, max: 64, step: 1 },
  { key: 'fadeIn', label: 'fadeIn', min: 0, max: 600, step: 0.01 },
  { key: 'fadeOut', label: 'fadeOut', min: 0, max: 600, step: 0.01 },
] as const
function update(key: typeof fields[number]['key'], event: Event) {
  const input = event.target as HTMLInputElement
  if (input.value.trim() && Number.isFinite(Number(input.value)) && project.selected) project.updateClip(project.selected.id, { [key]: Number(input.value) })
}
</script>

<template>
  <section v-if="project.selected" class="shrink-0 px-4 py-3 border-t border-border flex flex-col gap-3">
    <div class="flex items-center gap-2 text-xs">
      <span class="font-medium truncate min-w-0 flex-1" :title="project.selected.path">{{ project.selected.name }}</span>
      <Button variant="ghost" size="xs" @click="project.duplicateClip"><Copy />{{ t('project.duplicate') }}</Button>
      <Button variant="ghost" size="xs" @click="project.deleteClip"><Trash2 />{{ t('project.delete') }}</Button>
    </div>
    <div class="grid grid-cols-3 xl:grid-cols-6 gap-3">
      <label v-for="field in fields" :key="field.key" class="text-[11px] text-muted-foreground flex flex-col gap-1">
        {{ t(`project.${field.label}`) }}
        <input class="w-full min-w-0 bg-background border border-input rounded px-2 h-7 tabular text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring" type="number" :min="field.min" :max="field.max" :step="field.step" :value="Number(project.selected[field.key].toFixed(5))" @change="update(field.key, $event)">
      </label>
    </div>
  </section>
</template>
