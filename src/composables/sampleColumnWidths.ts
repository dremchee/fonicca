import type { ComputedRef, Ref } from 'vue'
import type { ResizableSampleColumnId, SampleColumnWidths, SAMPLE_COLUMNS } from '@/sample-columns'
import { useElementSize } from '@vueuse/core'
import { useSampleColumns } from '@/composables/sampleColumns'
import { computed, shallowRef } from 'vue'
import { boundedSampleColumnWidth, sampleTableLayout } from '@/sample-columns'

export function useSampleColumnWidths(columns: ComputedRef<readonly typeof SAMPLE_COLUMNS[number][]>, viewport: Ref<HTMLElement | null>) {
  const { width: viewportWidth } = useElementSize(viewport)
  const { widths: saved } = useSampleColumns()
  const draft = shallowRef<{ id: ResizableSampleColumnId, widths: SampleColumnWidths } | null>(null)
  const manualAtWidth = shallowRef<number | null>(null)
  const tableStyle = computed(() => sampleTableLayout(columns.value, draft.value?.widths ?? saved.value,
    viewportWidth.value, !draft.value && manualAtWidth.value !== viewportWidth.value))

  // Start from the widths actually on screen, including the flexible name.
  function renderedWidths(): SampleColumnWidths {
    const cells = viewport.value?.querySelector('.header')?.children
    const widths = { ...saved.value }
    if (cells) {
      const ids: ResizableSampleColumnId[] = ['name', ...columns.value.map(column => column.id)]
      ids.forEach((id, index) => {
        const size = cells[index + 1]?.getBoundingClientRect().width
        if (size) widths[id] = boundedSampleColumnWidth(id, size)
      })
    }
    return widths
  }

  function preview(id: ResizableSampleColumnId, width: number) {
    draft.value = { id, widths: { ...(draft.value?.widths ?? renderedWidths()), [id]: boundedSampleColumnWidth(id, width) } }
  }
  function commit(id: ResizableSampleColumnId, width: number) {
    saved.value = { ...(draft.value?.widths ?? renderedWidths()), [id]: boundedSampleColumnWidth(id, width) }
    manualAtWidth.value = viewportWidth.value
    draft.value = null
  }
  function cancel() {
    draft.value = null
  }
  function reset(id?: ResizableSampleColumnId) {
    manualAtWidth.value = null
    if (id) {
      const next = { ...saved.value }
      delete next[id]
      saved.value = next
    }
    else {
      saved.value = {}
    }
    cancel()
  }

  return { tableStyle, resizingColumn: computed(() => draft.value?.id), preview, commit, cancel, reset }
}
