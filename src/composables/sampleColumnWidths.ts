import type { ComputedRef, Ref } from 'vue'
import type { ResizableSampleColumnId, SAMPLE_COLUMNS } from '@/sample-columns'
import { useElementSize } from '@vueuse/core'
import { useSampleColumns } from '@/composables/sampleColumns'
import { useSampleTable } from '@/composables/sampleTable'
import { computed, shallowRef } from 'vue'
import { boundedSampleColumnWidth, sampleTableLayout } from '@/sample-columns'

export function useSampleColumnWidths(columns: ComputedRef<readonly typeof SAMPLE_COLUMNS[number][]>, viewport: Ref<HTMLElement | null>) {
  const { width: viewportWidth } = useElementSize(viewport)
  const { widths: saved } = useSampleColumns()
  const { table } = useSampleTable()
  const draft = shallowRef<{ id: ResizableSampleColumnId, width: number } | null>(null)
  const tableStyle = computed(() => sampleTableLayout(columns.value, draft.value
    ? { ...saved.value, [draft.value.id]: draft.value.width }
    : saved.value, viewportWidth.value))

  function preview(id: ResizableSampleColumnId, width: number) {
    draft.value = { id, width: boundedSampleColumnWidth(id, width) }
  }
  function commit(id: ResizableSampleColumnId, width: number) {
    table.setColumnSizing(previous => ({ ...previous, [id]: boundedSampleColumnWidth(id, width) }))
    draft.value = null
  }
  function cancel() {
    draft.value = null
  }
  function reset(id?: ResizableSampleColumnId) {
    if (id) {
      table.setColumnSizing((previous) => {
        const next = { ...previous }
        delete next[id]
        return next
      })
    }
    else {
      table.resetColumnSizing()
    }
    cancel()
  }

  return { tableStyle, resizingColumn: computed(() => draft.value?.id), preview, commit, cancel, reset }
}
