<script setup lang="ts">
import { PanelLeft } from '@lucide/vue'
import { useResizeObserver, useStorage, useTimeoutFn } from '@vueuse/core'
import { nextTick, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import SourceSidebar from '@/components/SourceSidebar.vue'
import { Button } from '@/components/ui/button'
import { createResizableController, ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { migrateLayout } from '@/components/ui/resizable/migrateLayout'
import { sidebarHidden } from '@/composables/sidebarVisibility'

defineProps<{ canToggle: boolean }>()
const { t } = useI18n()
const layoutId = 'fonicca-layout-columns'
migrateLayout(layoutId, [['sidebar', 'main']])
const controller = createResizableController()
const panel = useTemplateRef<HTMLElement>('panel')
const expandedWidth = useStorage('fonicca-sidebar-width', 210)
const animating = ref(false)

function rememberWidth() {
  const width = panel.value?.getBoundingClientRect().width ?? 0
  if (width === 0 && controller.isCollapsed('sidebar')) sidebarHidden.value = true
  if (!sidebarHidden.value && !animating.value && width > 0)
    expandedWidth.value = width
}
useResizeObserver(panel, rememberWidth)
const { start: finishAnimation } = useTimeoutFn(() => {
  animating.value = false
  rememberWidth()
}, 240, { immediate: false })

watch(sidebarHidden, async (hidden) => {
  // Keep both panels mounted so an interrupted transition reverses smoothly.
  animating.value = true
  await nextTick()
  if (hidden) controller.collapse('sidebar')
  else controller.expand('sidebar')
  finishAnimation()
})

function syncCollapsed() {
  sidebarHidden.value = controller.isCollapsed('sidebar')
}
</script>

<template>
  <div
    class="library-layout relative h-full"
    :class="{ 'sidebar-hidden': sidebarHidden, 'sidebar-animating': animating }"
    :style="{ '--sidebar-width': `${expandedWidth}px` }"
  >
    <ResizablePanelGroup
      :controller="controller" :auto-save-id="layoutId"
      class="h-full" :class="{ 'sidebar-animating': animating }" @layout-commit="syncCollapsed"
    >
      <ResizablePanel id="sidebar" :default-size="`${expandedWidth}px`" min-size="170px" max-size="320px" collapsible>
        <template #render="{ props }">
          <div ref="panel" v-bind="props" class="overflow-hidden">
            <SourceSidebar :style="{ width: animating || sidebarHidden ? `${expandedWidth}px` : '100%' }" />
          </div>
        </template>
      </ResizablePanel>
      <ResizableHandle
        :disabled="sidebarHidden" :aria-label="t('sidebar.resizeSidebar')"
        :class="{ 'opacity-0': sidebarHidden }" @pointerdown="animating = false"
      />
      <ResizablePanel id="main" min-size="520px">
        <template #render="{ props }">
          <main v-bind="props" class="flex min-w-0 flex-col"><slot /></main>
        </template>
      </ResizablePanel>
    </ResizablePanelGroup>
    <!-- Keep the toggle accessible while the panel shrinks. -->
    <div v-if="canToggle" class="sidebar-toggle absolute top-0 h-16 flex items-center border-b border-transparent" data-tauri-drag-region="deep">
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
.sidebar-animating > :deep([data-slot="resizable-panel"]) { transition: flex-basis 220ms cubic-bezier(0.32, 0.72, 0, 1); }
.sidebar-animating :deep(.library-toolbar) { transition: padding-left 220ms cubic-bezier(0.32, 0.72, 0, 1); }
.sidebar-animating :deep(.library-context) { transition: padding-left 220ms cubic-bezier(0.32, 0.72, 0, 1); }
@media (prefers-reduced-motion: reduce) {
  .sidebar-animating .sidebar-toggle,
  .sidebar-animating > :deep([data-slot="resizable-panel"]),
  .sidebar-animating :deep(.library-toolbar),
  .sidebar-animating :deep(.library-context) { transition: none; }
}
</style>
