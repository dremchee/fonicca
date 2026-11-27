<script setup lang="ts">
import { ChevronDown, Music2 } from '@lucide/vue'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@ui-sdk/vue/popover'
import { Toggle, ToggleGroup } from '@ui-sdk/vue/toggle'
import { computed, shallowRef, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import RenderChild from '@/components/ui/RenderChild'
import { toggleVariants } from '@/components/ui/toggle'

const key = defineModel<string | null>({ required: true })
const { t } = useI18n()
const notesEl = useTemplateRef<HTMLDivElement>('notesEl')
const mode = shallowRef<'major' | 'minor'>('major')
const note = computed(() => key.value?.replace(/m$/, '') ?? '')
const selectedNote = computed(() => note.value ? [note.value] : [])
const selectedMode = computed(() => [mode.value])
const choiceStyle = toggleVariants({ variant: 'outline', size: 'sm' })
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

function selectNote([value]: string[]) {
  key.value = value ? value + (mode.value === 'minor' ? 'm' : '') : null
}

function selectMode([value]: string[]) {
  if (value !== 'major' && value !== 'minor') return
  mode.value = value
  if (note.value) key.value = note.value + (value === 'minor' ? 'm' : '')
}

function initialFocus() {
  const notes = notesEl.value
  return notes?.querySelector<HTMLButtonElement>('[data-pressed]') ?? notes?.querySelector('button')
}
</script>

<template>
  <Popover>
    <PopoverTrigger :aria-label="t('filters.key')">
      <template #render="{ props }">
        <RenderChild :trigger-props="props">
          <Button variant="outline" size="sm" class="key-trigger min-w-24 justify-between font-normal">
            <Music2 data-icon="inline-start" class="text-muted-foreground" />
            <span>{{ key ?? t('filters.key') }}</span>
            <ChevronDown data-icon="inline-end" class="key-chevron" />
          </Button>
        </RenderChild>
      </template>
    </PopoverTrigger>
    <PopoverContent
      align="start" :side-offset="4" :initial-focus="initialFocus"
      class="key-popover ui-popup bg-popover text-popover-foreground w-80 rounded-2xl border p-4 shadow-md outline-hidden"
    >
      <div class="key-options">
        <div class="mb-3 flex items-center justify-between gap-2">
          <PopoverTitle class="text-sm font-medium">{{ t('filters.key') }}</PopoverTitle>
          <Button variant="ghost" size="xs" class="text-muted-foreground" :disabled="key === null" @click="key = null">
            {{ t('filters.anyKey') }}
          </Button>
        </div>
        <div ref="notesEl">
          <ToggleGroup class="key-notes" loop :aria-label="t('filters.note')" :value="selectedNote" @update:value="selectNote">
            <Toggle
              v-for="n in notes" :key="n.value" :value="n.value" :class="[choiceStyle, 'note-choice']"
              :style="{ gridColumn: `${n.column} / span 2`, gridRow: n.row }"
            >
              {{ n.value }}
            </Toggle>
          </ToggleGroup>
        </div>
        <ToggleGroup
          class="key-modes mt-4" :deselectable="false"
          :aria-label="t('filters.keyMode')" :value="selectedMode" @update:value="selectMode"
        >
          <Toggle value="major" :class="[choiceStyle, 'mode-choice']">{{ t('filters.major') }}</Toggle>
          <Toggle value="minor" :class="[choiceStyle, 'mode-choice']">{{ t('filters.minor') }}</Toggle>
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
.key-trigger[data-open] .key-chevron {
  transform: rotate(180deg);
}
.key-popover {
  margin: 0;
  max-width: min(var(--available-width, 100vw), calc(100vw - 1.5rem));
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
.key-options :deep(.note-choice[data-pressed]),
.key-options :deep(.mode-choice[data-pressed]) {
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
