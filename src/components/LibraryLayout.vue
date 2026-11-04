<script setup lang="ts">
import { PanelLeft } from '@lucide/vue'
import type { SplitterPanel } from 'reka-ui'
import { useResizeObserver, useStorage, useTimeoutFn } from '@vueuse/core'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import SourceSidebar from '@/components/SourceSidebar.vue'
import { Button } from '@/components/ui/button'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { sidebarHidden } from '@/composables/sidebarVisibility'

defineProps<{ canToggle: boolean }>()
const { t } = useI18n()
// The shadcn wrapper forwards the underlying SplitterPanel's imperative API.
const panel = useTemplateRef<InstanceType<typeof SplitterPanel>>('panel')
const panelElement = computed(() => panel.value?.$el as HTMLElement | undefined)
const expandedWidth = useStorage('fonicca-sidebar-width', 210)
const animating = ref(false)

function rememberWidth() {
  const width = panelElement.value?.getBoundingClientRect().width ?? 0
  if (!sidebarHidden.value && !animating.value && width > 0)
    expandedWidth.value = width
}
useResizeObserver(panelElement, rememberWidth)
const { start: finishAnimation } = useTimeoutFn(() => {
  animating.value = false
  rememberWidth()
}, 240, { immediate: false })

watch(sidebarHidden, async (hidden) => {
  // Keep both panels mounted so an interrupted transition reverses smoothly.
  animating.value = true
  await nextTick()
  if (hidden) panel.value?.collapse()
  else panel.value?.expand()
  finishAnimation()
})

function onDragging(dragging: boolean) {
  if (dragging) animating.value = false
}
</script>

<template>
  <div
    class="library-layout relative h-full"
    :class="{ 'sidebar-hidden': sidebarHidden, 'sidebar-animating': animating }"
    :style="{ '--sidebar-width': `${expandedWidth}px` }"
  >
    <ResizablePanelGroup direction="horizontal" auto-save-id="fonicca-layout-columns" class="h-full" :class="{ 'sidebar-animating': animating }">
      <ResizablePanel
        id="sidebar" ref="panel" :order="1" size-unit="px" :default-size="expandedWidth" :min-size="170" :max-size="320"
        collapsible :collapsed-size="0" :inert="sidebarHidden" :aria-hidden="sidebarHidden || undefined"
        @collapse="sidebarHidden = true" @expand="sidebarHidden = false"
      >
        <SourceSidebar :style="{ width: animating || sidebarHidden ? `${expandedWidth}px` : '100%' }" />
      </ResizablePanel>
      <ResizableHandle :disabled="sidebarHidden" :tabindex="sidebarHidden ? -1 : 0" :class="{ 'opacity-0': sidebarHidden }" @dragging="onDragging" />
      <ResizablePanel id="main" :order="2" as="main" size-unit="px" :min-size="520" class="min-w-0 flex flex-col">
        <slot />
      </ResizablePanel>
    </ResizablePanelGroup>
    <!-- Keep the toggle accessible while the panel shrinks. -->
    <div v-if="canToggle" class="sidebar-toggle absolute top-0 h-16 flex items-center border-b border-transparent">
      <Hint :text="t(sidebarHidden ? 'sidebar.showSidebar' : 'sidebar.hideSidebar')">
        <Button
          variant="ghost" size="icon-sm" :aria-label="t(sidebarHidden ? 'sidebar.showSidebar' : 'sidebar.hideSidebar')"
          :aria-expanded="!sidebarHidden" aria-controls="sidebar" @click="sidebarHidden = !sidebarHidden"
        ><PanelLeft /></Button>
      </Hint>
    </div>
  </div>
</template>

<style scoped>
.library-layout {
  --sidebar-toggle-left: calc(var(--sidebar-width) - 2rem - 0.5rem);
  --library-toolbar-inset: 0.75rem;
}
.sidebar-hidden {
  /* Leave room for the native macOS window controls. */
  --sidebar-toggle-left: 6rem;
  --library-toolbar-inset: calc(var(--sidebar-toggle-left) + 2rem + 0.75rem);
}
.sidebar-toggle { left: var(--sidebar-toggle-left); }
.sidebar-animating .sidebar-toggle { transition: left 220ms cubic-bezier(0.32, 0.72, 0, 1); }
.sidebar-animating > :deep([data-panel]) { transition: flex-grow 220ms cubic-bezier(0.32, 0.72, 0, 1); }
.sidebar-animating :deep(.library-toolbar) { transition: padding-left 220ms cubic-bezier(0.32, 0.72, 0, 1); }
.sidebar-animating :deep(.library-context) { transition: padding-left 220ms cubic-bezier(0.32, 0.72, 0, 1); }
@media (prefers-reduced-motion: reduce) {
  .sidebar-animating .sidebar-toggle,
  .sidebar-animating > :deep([data-panel]),
  .sidebar-animating :deep(.library-toolbar),
  .sidebar-animating :deep(.library-context) { transition: none; }
}
</style>
