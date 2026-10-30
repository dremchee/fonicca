<script setup lang="ts">
import { MoreHorizontal } from '@lucide/vue'
import { useResizeObserver } from '@vueuse/core'
import { computed, onBeforeUnmount, ref, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'

const props = defineProps<{ items: string[], overflowItems?: string[] }>()
const { t } = useI18n()
const container = ref<HTMLElement | null>(null)
const menu = ref<HTMLElement | null>(null)
const open = ref(false)
const available = ref(0)
const widths = ref<Record<string, number>>({})
const elements = new Map<string, HTMLElement>()
const observer = new ResizeObserver(entries => {
  const next = { ...widths.value }
  for (const entry of entries) {
    const id = (entry.target as HTMLElement).dataset.tool
    if (id) next[id] = entry.target.getBoundingClientRect().width
  }
  widths.value = next
})
function capture(id: string, element: unknown) {
  const previous = elements.get(id)
  if (previous === element) return
  if (previous) observer.unobserve(previous)
  if (element instanceof HTMLElement) {
    elements.set(id, element)
    observer.observe(element)
  } else elements.delete(id)
}
onBeforeUnmount(() => observer.disconnect())
useResizeObserver(container, ([entry]) => { available.value = entry?.contentRect.width ?? 0 })
const orderedItems = computed(() => [...props.items.filter(id => !props.overflowItems?.includes(id)), ...props.items.filter(id => props.overflowItems?.includes(id))])
const visibleCount = computed(() => {
  const primaryItems = orderedItems.value.filter(id => !props.overflowItems?.includes(id))
  const sizes = primaryItems.map(id => (widths.value[id] ?? 32) + 4)
  if (sizes.reduce((sum, width) => sum + width, 0) + (props.overflowItems?.length ? 36 : 0) <= available.value) return sizes.length
  let used = 36 // Reserve the overflow trigger before admitting more tools.
  let count = 0
  for (const width of sizes) {
    if (used + width > available.value) break
    used += width
    count++
  }
  return count
})
const overflow = computed(() => visibleCount.value < props.items.length)
watchEffect(() => { if (!overflow.value) open.value = false })
</script>

<template>
  <div ref="container" class="relative min-w-8 min-h-8 flex-1 flex items-center gap-1">
    <Teleport v-for="(item, index) in orderedItems" :key="item" :to="menu ?? 'body'" :disabled="index < visibleCount || !menu">
      <div
        :ref="element => capture(item, element)" :data-tool="item"
        class="w-max shrink-0 inline-flex items-center"
        :class="{ 'overflow-tool-parked': index >= visibleCount && !menu }"
        :inert="index >= visibleCount && !menu"
      >
        <slot :item="item" />
      </div>
    </Teleport>
    <Popover v-if="overflow" v-model:open="open">
      <PopoverTrigger as-child>
        <Button variant="ghost" size="icon-sm" class="shrink-0 data-[state=open]:bg-accent" :aria-label="t('editor.toolbar.moreTools')"><MoreHorizontal /></Button>
      </PopoverTrigger>
      <PopoverContent align="end" class="w-auto max-w-80 p-2" :aria-label="t('editor.toolbar.moreTools')">
        <div ref="menu" class="flex flex-wrap items-center gap-1" />
      </PopoverContent>
    </Popover>
  </div>
</template>

<style scoped>
.overflow-tool-parked { position: absolute; visibility: hidden; pointer-events: none; }
</style>
