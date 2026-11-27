<script setup lang="ts">
import { useEventListener, useMutationObserver, useTimeoutFn } from '@vueuse/core'
import { computed, onScopeDispose, shallowRef, useTemplateRef, watch } from 'vue'
import { hintLines } from '@/hint-text'
import { Kbd } from '@/components/ui/kbd'
import { createTooltipController, Tooltip, TooltipContent } from '@/components/ui/tooltip'

// The list delegates pointer and keyboard events to one tooltip per viewport.
const props = defineProps<{ viewport: HTMLElement | null }>()
const tooltip = createTooltipController()
const anchor = shallowRef<HTMLElement>()
const label = useTemplateRef<HTMLElement>('label')
const text = shallowRef('')
const lines = computed(() => hintLines(text.value))
const shortcut = computed(() => anchor.value?.dataset.hintShortcut)
const side = computed(() => anchor.value?.dataset.hintSide === 'bottom' ? 'bottom' : 'top')
const open = shallowRef(false)
let hovered: HTMLElement | undefined

function show(element: HTMLElement) {
  if (!props.viewport?.contains(element)) return
  clearCloseTimer()
  anchor.value = element
  text.value = element.dataset.sampleHint ?? ''
  tooltip.open({ anchor: element })
}

const { start: scheduleOpen, stop: clearOpenTimer } = useTimeoutFn(() => {
  if (hovered) show(hovered)
}, 500, { immediate: false })
const { start: scheduleClose, stop: clearCloseTimer } = useTimeoutFn(() => tooltip.close(), 160, { immediate: false })

function dismiss() {
  clearOpenTimer()
  clearCloseTimer()
  hovered = undefined
  tooltip.close()
}

function hintTarget(target: EventTarget | null) {
  const element = target instanceof Element ? target.closest<HTMLElement>('[data-sample-hint]') : null
  return element?.dataset.sampleHint && props.viewport?.contains(element) ? element : undefined
}

function onTooltipLeave(event: PointerEvent) {
  // The native popover can briefly pass under the pointer while being placed.
  // Returning from the popup to its anchor should keep the hint open.
  const element = hintTarget(event.relatedTarget)
  if (element && element === anchor.value) {
    hovered = element
    clearCloseTimer()
  }
  else scheduleClose()
}

useEventListener(() => props.viewport, 'pointermove', (event: PointerEvent) => {
  if (event.pointerType === 'touch') return
  const element = hintTarget(event.target)
  if (element === hovered) return
  hovered = element
  clearOpenTimer()
  if (element) {
    clearCloseTimer()
    if (open.value) show(element)
    else scheduleOpen()
  }
  else if (open.value) scheduleClose()
})
useEventListener(() => props.viewport, 'pointerleave', () => {
  hovered = undefined
  clearOpenTimer()
  if (open.value) scheduleClose()
})
useEventListener(() => props.viewport, 'focusin', (event: FocusEvent) => {
  const element = hintTarget(event.target)
  if (element?.matches(':focus-visible')) show(element)
})
useEventListener(() => props.viewport, 'focusout', dismiss)
useEventListener(() => props.viewport, 'pointerdown', dismiss, { capture: true })
useEventListener(() => props.viewport, 'contextmenu', dismiss, { capture: true })
useEventListener(() => props.viewport, 'scroll', dismiss, { passive: true })
useEventListener(window, 'blur', dismiss)

useMutationObserver(() => props.viewport, () => {
  if (open.value && anchor.value && (!props.viewport?.contains(anchor.value) || anchor.value.dataset.sampleHint !== text.value)) dismiss()
}, { subtree: true, childList: true, attributes: true, attributeFilter: ['data-sample-hint'] })

watch([anchor, open, label], ([element, visible, textElement], _, onCleanup) => {
  const id = textElement?.closest('[role="tooltip"]')?.id
  if (!element || !visible || !id) return
  const ids = new Set(element.getAttribute('aria-describedby')?.split(/\s+/).filter(Boolean))
  ids.add(id)
  element.setAttribute('aria-describedby', [...ids].join(' '))
  onCleanup(() => {
    const remaining = element.getAttribute('aria-describedby')?.split(/\s+/).filter(token => token && token !== id)
    if (remaining?.length) element.setAttribute('aria-describedby', remaining.join(' '))
    else element.removeAttribute('aria-describedby')
  })
}, { flush: 'post' })
onScopeDispose(dismiss)
</script>

<template>
  <Tooltip :controller="tooltip" @open-change="open = $event">
    <TooltipContent
      :side="side" data-library-hint class="flex max-w-64 items-center justify-center gap-2"
      @pointerenter="clearCloseTimer" @pointerleave="onTooltipLeave"
    >
      <span ref="label" class="min-w-0 [overflow-wrap:anywhere] [text-wrap:wrap]">
        <span v-for="(line, index) in lines" :key="index" class="block">{{ line }}</span>
      </span>
      <Kbd v-if="shortcut" class="shrink-0">{{ shortcut }}</Kbd>
    </TooltipContent>
  </Tooltip>
</template>
