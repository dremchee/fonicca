<script setup lang="ts">
import type { AcceptableValue } from 'reka-ui'
import { ChevronDown, Music2 } from '@lucide/vue'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'

const key = defineModel<string | null>({ required: true })
const { t } = useI18n()
const notesEl = useTemplateRef<HTMLDivElement>('notesEl')
const mode = ref<'major' | 'minor'>('major')
const note = computed(() => key.value?.replace(/m$/, '') ?? '')
const notes = [
  { value: 'C', column: 1, row: 2 },
  { value: 'C#', column: 2, row: 1 },
  { value: 'D', column: 3, row: 2 },
  { value: 'D#', column: 4, row: 1 },
  { value: 'E', column: 5, row: 2 },
  { value: 'F', column: 7, row: 2 },
  { value: 'F#', column: 8, row: 1 },
  { value: 'G', column: 9, row: 2 },
  { value: 'G#', column: 10, row: 1 },
  { value: 'A', column: 11, row: 2 },
  { value: 'A#', column: 12, row: 1 },
  { value: 'B', column: 13, row: 2 },
]

// Mode alone is a preference; the existing query still takes one complete key.
watch(key, (value) => {
  if (value)
    mode.value = value.endsWith('m') ? 'minor' : 'major'
}, { immediate: true, flush: 'sync' })

function selectNote(value: AcceptableValue | AcceptableValue[]) {
  if (!value) key.value = null
  else if (typeof value === 'string') key.value = value + (mode.value === 'minor' ? 'm' : '')
}

function selectMode(value: AcceptableValue | AcceptableValue[]) {
  // Keep one mode selected when its active segment is clicked again.
  if (value !== 'major' && value !== 'minor') return
  mode.value = value
  if (note.value) key.value = note.value + (value === 'minor' ? 'm' : '')
}

function focusNote(event: Event) {
  event.preventDefault()
  void nextTick(() => {
    const notes = notesEl.value
    const selected = notes?.querySelector<HTMLButtonElement>('[data-state="on"]')
    ;(selected ?? notes?.querySelector<HTMLButtonElement>('button'))?.focus()
  })
}
</script>

<template>
  <Popover>
    <PopoverTrigger as-child>
      <Button variant="outline" size="sm" class="key-trigger min-w-24 justify-between font-normal" :aria-label="t('filters.key')">
        <Music2 data-icon="inline-start" class="text-muted-foreground" />
        <span>{{ key ?? t('filters.key') }}</span>
        <ChevronDown data-icon="inline-end" class="key-chevron" />
      </Button>
    </PopoverTrigger>
    <PopoverContent align="start" class="w-80 rounded-2xl p-4" :aria-label="t('filters.key')" @open-auto-focus="focusNote">
      <div class="key-options">
        <div class="mb-3 flex items-center justify-between gap-2">
          <span class="text-sm font-medium">{{ t('filters.key') }}</span>
          <Button variant="ghost" size="xs" class="text-muted-foreground" :disabled="key === null" @click="key = null">
            {{ t('filters.anyKey') }}
          </Button>
        </div>
        <div ref="notesEl">
          <ToggleGroup
            type="single" variant="outline" size="sm" :spacing="1" class="key-notes" loop
            :aria-label="t('filters.note')" :model-value="note" @update:model-value="selectNote"
          >
            <ToggleGroupItem
              v-for="n in notes" :key="n.value" :value="n.value" class="note-choice"
              :style="{ gridColumn: `${n.column} / span 2`, gridRow: n.row }"
            >
              {{ n.value }}
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
        <ToggleGroup
          type="single" variant="outline" size="sm" :spacing="2" class="key-modes mt-4"
          :aria-label="t('filters.keyMode')" :model-value="mode" @update:model-value="selectMode"
        >
          <ToggleGroupItem value="major" class="mode-choice">{{ t('filters.major') }}</ToggleGroupItem>
          <ToggleGroupItem value="minor" class="mode-choice">{{ t('filters.minor') }}</ToggleGroupItem>
        </ToggleGroup>
      </div>
    </PopoverContent>
  </Popover>
</template>

<style scoped>
.key-chevron {
  opacity: 0.5;
  transition: transform 120ms ease;
}
.key-trigger[data-state='open'] .key-chevron {
  transform: rotate(180deg);
}
.key-options :deep(.key-notes) {
  display: grid;
  width: 100%;
  grid-template-columns: repeat(14, minmax(0, 1fr));
  gap: 0.5rem 0.25rem;
}
.key-options :deep(.note-choice) {
  width: 100%;
  height: auto;
  aspect-ratio: 1;
  padding: 0;
  border-radius: 50%;
  corner-shape: round;
  font-weight: 600;
}
.key-options :deep(.note-choice[data-state='on']),
.key-options :deep(.mode-choice[data-state='on']) {
  background: var(--primary);
  color: var(--primary-foreground);
  border-color: var(--primary);
}
.key-options :deep(.key-modes) {
  display: flex;
  width: 100%;
  gap: 0.5rem;
}
.key-options :deep(.mode-choice) {
  flex: 1;
  border-radius: 9999px;
  corner-shape: round;
}
@media (prefers-reduced-motion: reduce) {
  .key-chevron { transition: none; }
}
</style>
