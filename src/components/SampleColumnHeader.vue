<script setup lang="ts">
import { useEventListener, useResizeObserver } from '@vueuse/core'
import { onBeforeUnmount, shallowRef, useTemplateRef } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{ label: string, minWidth: number, maxWidth: number, alignRight?: boolean }>()
const emit = defineEmits<{ resize: [width: number], commit: [width: number], cancel: [], reset: [] }>()
const { t } = useI18n()
const cell = useTemplateRef<HTMLElement>('cell')
const width = shallowRef(0)
const drag = shallowRef<{ pointerId: number, x: number, width: number } | null>(null)
useResizeObserver(cell, entries => width.value = Math.round(entries[0].contentRect.width))

function bound(value: number) {
  return Math.min(props.maxWidth, Math.max(props.minWidth, value))
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
    case 'Home': next = props.minWidth; break
    case 'End': next = props.maxWidth; break
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
  <div ref="cell" class="column-heading relative min-w-0 flex items-center h-full" :class="{ 'justify-end': alignRight }">
    <span class="truncate" :title="label">{{ label }}</span>
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
.resize-handle {
  position: absolute;
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
