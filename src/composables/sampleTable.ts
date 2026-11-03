import type { Column, ColumnDef, SortingState, Table } from '@tanstack/vue-table'
import type { Sample } from '@/api'
import {
  columnSizingFeature, columnVisibilityFeature, rowSortingFeature, tableFeatures, useTable,
} from '@tanstack/vue-table'
import { createGlobalState } from '@vueuse/core'
import { computed } from 'vue'
import { useSampleColumns } from '@/composables/sampleColumns'
import {
  boundedSampleColumnWidth, defaultSampleColumns, normalizeSampleSort, sampleColumnLimits,
  sampleDirectory, SAMPLE_COLUMNS,
} from '@/sample-columns'
import { useLibraryStore } from '@/stores/library'

const features = tableFeatures({ columnVisibilityFeature, columnSizingFeature, rowSortingFeature })
export type SampleTable = Table<typeof features, Sample>
export type SampleTableColumn = Column<typeof features, Sample>

const columns: ColumnDef<typeof features, Sample>[] = [
  {
    id: 'name', accessorKey: 'name', enableHiding: false, size: 220,
    minSize: sampleColumnLimits('name').min, maxSize: sampleColumnLimits('name').max,
  },
  ...SAMPLE_COLUMNS.map(column => ({
    id: column.id,
    accessorFn: (sample: Sample) => {
      switch (column.id) {
        case 'directory': return sampleDirectory(sample.path)
        case 'length': return sample.duration
        case 'type': return sample.ext
        default: return sample[column.id]
      }
    },
    enableSorting: column.id !== 'directory',
    size: column.width,
    minSize: sampleColumnLimits(column.id).min,
    maxSize: sampleColumnLimits(column.id).max,
  })),
]

/** Shared column controller; sparse row loading and virtualization stay in SampleList. */
export const useSampleTable = createGlobalState(() => {
  const library = useLibraryStore()
  const { visibility, widths } = useSampleColumns()
  const sorting = computed<SortingState>(() => library.scope.kind === 'similar'
    ? []
    : [{ id: library.sort.by, desc: library.sort.direction === 'desc' }])

  const table = useTable({
    features,
    columns,
    // Do not materialize the library's unloaded rows just to manage its columns.
    data: [],
    manualSorting: true,
    enableMultiSort: false,
    enableSortingRemoval: false,
    get enableSorting() { return library.scope.kind !== 'similar' },
    initialState: { columnVisibility: defaultSampleColumns() },
    state: {
      get columnVisibility() { return visibility.value },
      get columnSizing() { return widths.value },
      get sorting() { return sorting.value },
    },
    onColumnVisibilityChange: (updater) => {
      const next = typeof updater === 'function' ? updater(visibility.value) : updater
      visibility.value = Object.fromEntries(SAMPLE_COLUMNS.map(column => [column.id, next[column.id] !== false])) as typeof visibility.value
    },
    onColumnSizingChange: (updater) => {
      const next = typeof updater === 'function' ? updater(widths.value) : updater
      widths.value = Object.fromEntries(columns.flatMap((column) => {
        const id = column.id as 'name' | typeof SAMPLE_COLUMNS[number]['id']
        return next[id] == null ? [] : [[id, boundedSampleColumnWidth(id, next[id])]]
      }))
    },
    onSortingChange: (updater) => {
      const next = typeof updater === 'function' ? updater(sorting.value) : updater
      if (!next[0]) return
      const sort = normalizeSampleSort({ by: next[0].id, direction: next[0].desc ? 'desc' : 'asc' })
      library.setSort(sort.by, sort.direction)
    },
  })

  const visibleColumns = computed(() => {
    const visible = new Set(table.getVisibleLeafColumns().map(column => column.id))
    return SAMPLE_COLUMNS.filter(column => visible.has(column.id))
  })

  function resetWidths() {
    table.resetColumnSizing()
  }

  function resetView() {
    table.resetColumnVisibility()
    resetWidths()
  }

  return { table, visibleColumns, resetWidths, resetView }
})
