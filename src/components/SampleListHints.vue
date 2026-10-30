<script setup lang="ts">
import { useEventListener, useMutationObserver } from '@vueuse/core'
import { injectTooltipRootContext } from 'reka-ui'
import { computed, onScopeDispose, shallowRef, useId, watch } from 'vue'
import { hintLines } from '@/hint-text'
import { Kbd } from '@/components/ui/kbd'
import { TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

// One tooltip for the virtualized list. Cells remain plain elements, so changing
// the viewport does not mount a tooltip/trigger/popper tree for every cell.
const props = defineProps<{ viewport: HTMLElement | null }>()
const tooltip = injectTooltipRootContext()
tooltip.contentId ||= `sample-hint-${useId()}`
const anchor = shallowRef<HTMLElement>()
let hovered: HTMLElement | undefined
const text = computed(() => anchor.value?.dataset.sampleHint ?? '')
const lines = computed(() => hintLines(text.value))
const shortcut = computed(() => anchor.value?.dataset.hintShortcut)
const side = computed(() => anchor.value?.dataset.hintSide === 'bottom' ? 'bottom' : 'top')
// The hidden trigger provides Reka's public positioning reference. Its default
// DOM anchor is replaced with the real cell for hover grace and scroll dismissal.
watch(anchor, element => tooltip.onTriggerChange(element), { flush: 'post' })

function target(event: Event) {
  const element = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-sample-hint]') : null
  return element?.dataset.sampleHint && props.viewport?.contains(element) ? element : undefined
}
function activate(element: HTMLElement, keyboard: boolean) {
  if (tooltip.disabled.value) return
  if (anchor.value !== element) tooltip.onClose()
  anchor.value = element
  tooltip.onTriggerChange(element)
  if (keyboard) tooltip.onOpen()
  else tooltip.onTriggerEnter()
}
function dismiss() {
  hovered = undefined
  tooltip.onClose()
  anchor.value = undefined
  tooltip.onTriggerChange(undefined)
}
useEventListener(() => props.viewport, 'pointermove', (event: PointerEvent) => {
  if (event.pointerType === 'touch') return
  const element = target(event)
  if (element === hovered) return
  if (hovered) tooltip.onTriggerLeave()
  hovered = element
  if (element) activate(element, false)
})
useEventListener(() => props.viewport, 'pointerleave', () => {
  hovered = undefined
  // The existing tooltip grace area lets the pointer move onto its content.
  tooltip.onTriggerLeave()
})
useEventListener(() => props.viewport, 'focusin', (event: FocusEvent) => {
  const element = target(event)
  if (element?.matches(':focus-visible')) activate(element, true)
})
useEventListener(() => props.viewport, 'focusout', dismiss)
useEventListener(() => props.viewport, 'pointerdown', dismiss, { capture: true })
useEventListener(() => props.viewport, 'contextmenu', dismiss, { capture: true })
useEventListener(() => props.viewport, 'scroll', dismiss, { passive: true })
useEventListener(window, 'blur', dismiss)

// A refreshed/evicted row must not leave an open tooltip attached to old data.
useMutationObserver(() => props.viewport, () => {
  if (anchor.value && (!props.viewport?.contains(anchor.value) || anchor.value.dataset.sampleHint !== text.value)) dismiss()
}, { subtree: true, childList: true, attributes: true, attributeFilter: ['data-sample-hint'] })

// Preserve any existing descriptions. Reka still owns Escape, hoverable content,
// outside dismissal and placement; its trigger is the actual cell/button.
watch([anchor, tooltip.open], ([element, open], _, onCleanup) => {
  if (!element || !open) return
  const id = tooltip.contentId
  const ids = new Set(element.getAttribute('aria-describedby')?.split(/\s+/).filter(Boolean))
  ids.add(id)
  element.setAttribute('aria-describedby', [...ids].join(' '))
  onCleanup(() => {
    const remaining = element.getAttribute('aria-describedby')?.split(/\s+/).filter(token => token && token !== id)
    if (remaining?.length) element.setAttribute('aria-describedby', remaining.join(' '))
    else element.removeAttribute('aria-describedby')
  })
}, { flush: 'sync' })
onScopeDispose(dismiss)
</script>

<template>
  <TooltipTrigger :reference="anchor" as="span" hidden aria-hidden="true" />
  <TooltipContent :side="side" class="flex max-w-64 items-center justify-center gap-2">
    <span class="min-w-0 [overflow-wrap:anywhere] [text-wrap:wrap]">
      <span v-for="(line, index) in lines" :key="index" class="block">{{ line }}</span>
    </span>
    <Kbd v-if="shortcut" class="shrink-0">{{ shortcut }}</Kbd>
  </TooltipContent>
</template>
