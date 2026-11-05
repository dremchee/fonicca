<script setup lang="ts">
import type { SampleTableHeader } from '@/composables/sampleTable'
import type { ResizableSampleColumnId, SampleColumnWidths } from '@/sample-columns'
import { ArrowDown, ArrowUp, ArrowUpDown, EyeOff, RotateCcw } from '@lucide/vue'
import { useEventListener, useResizeObserver } from '@vueuse/core'
import { computed, onBeforeUnmount, shallowRef, useTemplateRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { useSampleTable } from '@/composables/sampleTable'
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem,
  DropdownMenuSeparator, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

const props = defineProps<{ header: SampleTableHeader, label: string, alignRight?: boolean }>()
const column = computed(() => props.header.column)
const { table, beginResize, finishResize } = useSampleTable()
const { t } = useI18n()
const direction = computed(() => column.value.getIsSorted())
const menuLabel = computed(() => [
  t('list.columns.options', { name: props.label }),
  direction.value ? t(direction.value === 'asc' ? 'list.columns.sortedAscending' : 'list.columns.sortedDescending') : '',
].filter(Boolean).join(', '))
const minWidth = computed(() => column.value.columnDef.minSize ?? 44)
const maxWidth = computed(() => column.value.columnDef.maxSize ?? 800)
const cell = useTemplateRef<HTMLElement>('cell')
const width = shallowRef(0)
let cancelDrag: (() => void) | undefined
useResizeObserver(cell, entries => width.value = Math.round(entries[0].contentRect.width))

function bound(value: number) {
  return Math.min(maxWidth.value, Math.max(minWidth.value, value))
}
function renderedWidths(): SampleColumnWidths {
  return Object.fromEntries(Array.from(cell.value?.closest('.table-content')?.querySelectorAll<HTMLElement>('.column-heading') ?? [])
    .map(heading => [heading.dataset.column, Math.round(heading.getBoundingClientRect().width)]))
}
function start(e: MouseEvent | TouchEvent) {
  if (!cell.value || cancelDrag || !column.value.getCanResize()) return
  if ('touches' in e ? e.touches.length !== 1 : e.button !== 0) return
  e.preventDefault()
  ;(e.currentTarget as HTMLElement).focus({ preventScroll: true })
  const initialWidth = Math.round(cell.value.getBoundingClientRect().width)
  beginResize(column.value.id as ResizableSampleColumnId, renderedWidths())

  const doc = cell.value.ownerDocument
  const listeners = new AbortController()
  let endNative: (() => void) | undefined
  // TanStack accepts a document context. Retain its end handler so Esc, blur,
  // and unmount can also release its document listeners and pending RAF.
  const context = {
    addEventListener(type: string, listener: EventListener, options?: AddEventListenerOptions) {
      if (type === 'mouseup' || type === 'touchend') endNative = () => listener(e)
      doc.addEventListener(type, listener, options)
    },
    removeEventListener: doc.removeEventListener.bind(doc),
  } as Document
  props.header.getResizeHandler(context)(e)

  function finish(cancelled = false) {
    listeners.abort()
    cancelDrag = undefined
    finishResize(cancelled || Math.round(cell.value?.getBoundingClientRect().width ?? 0) === initialWidth)
  }
  // Registered after TanStack, so its final sizing update runs before saving.
  doc.addEventListener('mouseup', () => finish(), { signal: listeners.signal })
  doc.addEventListener('touchend', () => finish(), { signal: listeners.signal })
  doc.addEventListener('touchcancel', () => finish(true), { signal: listeners.signal })
  cancelDrag = () => {
    endNative?.()
    finish(true)
  }
}
function cancel() {
  cancelDrag?.()
}
function onKeydown(e: KeyboardEvent) {
  if (e.metaKey || e.ctrlKey || e.altKey)
    return
  if (e.key === 'Escape' && cancelDrag) {
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
  if (cancelDrag) return
  beginResize(column.value.id as ResizableSampleColumnId, renderedWidths())
  table.setColumnSizing(previous => ({ ...previous, [column.value.id]: bound(next) }))
  finishResize()
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
          <DropdownMenuItem v-if="column.getCanResize()" @select="column.resetSize()">
            <RotateCcw /> {{ t('list.columns.resetWidth') }}
          </DropdownMenuItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
    <div
      v-if="column.getCanResize()" class="resize-handle" :class="{ resizing: column.getIsResizing() }"
      role="separator" tabindex="0" aria-orientation="vertical"
      :aria-label="t('list.columns.resize', { name: label })"
      :aria-valuemin="minWidth" :aria-valuemax="maxWidth" :aria-valuenow="width"
      :title="t('list.columns.resizeHint')"
      @mousedown.stop="start" @touchstart.stop="start" @keydown="onKeydown"
      @dblclick.stop.prevent="column.resetSize()"
    />
    <span v-else class="column-divider" aria-hidden="true" />
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
.resize-handle,
.column-divider {
  position: absolute;
  inset-block: 0;
  right: -12px;
  width: 12px;
}
.column-divider { pointer-events: none; }
.resize-handle {
  z-index: 1;
  cursor: col-resize;
  touch-action: none;
  outline: none;
}
.resize-handle::after,
.column-divider::after {
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
