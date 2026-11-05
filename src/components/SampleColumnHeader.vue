<script setup lang="ts">
import type { SampleTableColumn } from '@/composables/sampleTable'
import { ArrowDown, ArrowUp, ArrowUpDown, EyeOff, RotateCcw } from '@lucide/vue'
import { useEventListener, useResizeObserver } from '@vueuse/core'
import { computed, onBeforeUnmount, shallowRef, useTemplateRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem,
  DropdownMenuSeparator, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

const props = defineProps<{ column: SampleTableColumn, label: string, alignRight?: boolean }>()
const emit = defineEmits<{ resize: [width: number], commit: [width: number], cancel: [], reset: [] }>()
const { t } = useI18n()
const direction = computed(() => props.column.getIsSorted())
const menuLabel = computed(() => [
  t('list.columns.options', { name: props.label }),
  direction.value ? t(direction.value === 'asc' ? 'list.columns.sortedAscending' : 'list.columns.sortedDescending') : '',
].filter(Boolean).join(', '))
const minWidth = computed(() => props.column.columnDef.minSize ?? 44)
const maxWidth = computed(() => props.column.columnDef.maxSize ?? 800)
const cell = useTemplateRef<HTMLElement>('cell')
const width = shallowRef(0)
const drag = shallowRef<{ pointerId: number, x: number, width: number } | null>(null)
useResizeObserver(cell, entries => width.value = Math.round(entries[0].contentRect.width))

function bound(value: number) {
  return Math.min(maxWidth.value, Math.max(minWidth.value, value))
}
function start(e: PointerEvent) {
  if (e.button !== 0 || !e.isPrimary || !cell.value)
    return
  e.preventDefault()
  drag.value = { pointerId: e.pointerId, x: e.clientX, width: cell.value.getBoundingClientRect().width }
  const handle = e.currentTarget as HTMLElement
  handle.focus({ preventScroll: true })
  handle.setPointerCapture(e.pointerId)
}
function move(e: PointerEvent) {
  const start = drag.value
  if (start?.pointerId === e.pointerId)
    emit('resize', bound(start.width + e.clientX - start.x))
}
function finish(e: PointerEvent) {
  const start = drag.value
  if (start?.pointerId !== e.pointerId)
    return
  drag.value = null
  emit('commit', bound(start.width + e.clientX - start.x))
}
function cancel() {
  if (!drag.value)
    return
  drag.value = null
  emit('cancel')
}
function onKeydown(e: KeyboardEvent) {
  if (e.metaKey || e.ctrlKey || e.altKey)
    return
  if (e.key === 'Escape' && drag.value) {
    e.preventDefault()
    e.stopPropagation()
    cancel()
    return
  }
  const step = e.shiftKey ? 40 : 10
  let next: number
  switch (e.key) {
    case 'ArrowLeft': next = width.value - step; break
    case 'ArrowRight': next = width.value + step; break
    case 'Home': next = minWidth.value; break
    case 'End': next = maxWidth.value; break
    default: return
  }
  e.preventDefault()
  e.stopPropagation()
  emit('commit', bound(next))
}
useEventListener(window, 'blur', cancel)
onBeforeUnmount(cancel)
</script>

<template>
  <div
    ref="cell" class="column-heading relative min-w-0 flex items-center h-full" :class="{ 'justify-end': alignRight }"
    :data-column="column.id"
  >
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button
          variant="ghost" size="xs" class="sort-heading min-w-0 data-[state=open]:bg-accent"
          :class="{ 'text-foreground': direction, 'justify-end': alignRight, 'justify-start': !alignRight }"
          :aria-label="menuLabel"
        >
          <span class="truncate">{{ label }}</span>
          <component
            :is="direction === 'desc' ? ArrowDown : direction === 'asc' ? ArrowUp : ArrowUpDown"
            v-if="column.getCanSort()" data-icon="inline-end" class="text-muted-foreground" aria-hidden="true"
          />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent :align="alignRight ? 'end' : 'start'" class="normal-case tracking-normal">
        <DropdownMenuGroup v-if="column.getCanSort()">
          <DropdownMenuItem @select="column.toggleSorting(false)">
            <ArrowUp /> {{ t('list.columns.ascending') }}
          </DropdownMenuItem>
          <DropdownMenuItem @select="column.toggleSorting(true)">
            <ArrowDown /> {{ t('list.columns.descending') }}
          </DropdownMenuItem>
        </DropdownMenuGroup>
        <DropdownMenuSeparator v-if="column.getCanSort()" />
        <DropdownMenuGroup>
          <DropdownMenuItem v-if="column.getCanHide()" @select="column.toggleVisibility(false)">
            <EyeOff /> {{ t('list.columns.hide') }}
          </DropdownMenuItem>
          <DropdownMenuItem @select="emit('reset')">
            <RotateCcw /> {{ t('list.columns.resetWidth') }}
          </DropdownMenuItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
    <div
      class="resize-handle" :class="{ resizing: drag !== null }"
      role="separator" tabindex="0" aria-orientation="vertical"
      :aria-label="t('list.columns.resize', { name: label })"
      :aria-valuemin="minWidth" :aria-valuemax="maxWidth" :aria-valuenow="width"
      :title="t('list.columns.resizeHint')"
      @pointerdown.stop="start" @pointermove="move" @pointerup="finish"
      @pointercancel="cancel" @lostpointercapture="cancel" @keydown="onKeydown"
      @dblclick.stop.prevent="emit('reset')"
    />
  </div>
</template>

<style scoped>
.sort-heading {
  position: absolute;
  inset-block: 0;
  inset-inline: -6px;
  width: calc(100% + 12px);
  height: 100%;
  max-width: none;
  padding-inline: 6px;
  border-radius: 0;
  font: inherit;
  letter-spacing: inherit;
  text-transform: inherit;
}
.resize-handle {
  position: absolute;
  z-index: 1;
  inset-block: 0;
  right: -12px;
  width: 12px;
  cursor: col-resize;
  touch-action: none;
  outline: none;
}
.resize-handle::after {
  content: '';
  position: absolute;
  inset-block: 0;
  left: 5px;
  width: 1px;
  background: var(--border);
}
.resize-handle:hover::after,
.resize-handle:focus-visible::after,
.resize-handle.resizing::after {
  background: var(--primary);
}
.resize-handle:focus-visible::after {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
</style>
