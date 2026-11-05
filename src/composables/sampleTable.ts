import type { Column, ColumnDef, Header, SortingState, Table } from '@tanstack/vue-table'
import type { Sample } from '@/api'
import {
  columnResizingFeature, columnSizingFeature, columnVisibilityFeature, makeStateUpdater, rowSortingFeature, tableFeatures, useTable,
} from '@tanstack/vue-table'
import { createGlobalState } from '@vueuse/core'
import { computed, shallowRef, watch } from 'vue'
import { useSampleColumns } from '@/composables/sampleColumns'
import {
  boundedSampleColumnWidth, defaultSampleColumns, normalizeSampleSort, sampleColumnLimits, sampleColumnResizeMax,
  sampleDirectory, sampleTableLayout, SAMPLE_COLUMNS,
} from '@/sample-columns'
import type { ResizableSampleColumnId, SampleColumnWidths } from '@/sample-columns'
import { useLibraryStore } from '@/stores/library'

const features = tableFeatures({ columnVisibilityFeature, columnSizingFeature, columnResizingFeature, rowSortingFeature })
export type SampleTable = Table<typeof features, Sample>
export type SampleTableColumn = Column<typeof features, Sample>
export type SampleTableHeader = Header<typeof features, Sample>

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
  let resizeStart: SampleColumnWidths | null = null
  let resizingColumn: ResizableSampleColumnId | undefined
  const viewportWidth = shallowRef(0)
  const layoutColumns = computed(() => SAMPLE_COLUMNS.filter(column => visibility.value[column.id] !== false))
  const tableColumns = computed(() => {
    const last = layoutColumns.value.at(-1)?.id ?? 'name'
    return columns.map(column => ({
      ...column,
      enableResizing: column.id !== last,
      maxSize: column.id === last && last !== 'name'
        ? SAMPLE_COLUMNS.find(item => item.id === last)!.width
        : sampleColumnResizeMax(column.id as ResizableSampleColumnId, layoutColumns.value, viewportWidth.value),
    }))
  })
  const sorting = computed<SortingState>(() => library.scope.kind === 'similar'
    ? []
    : [{ id: library.sort.by, desc: library.sort.direction === 'desc' }])

  const table = useTable({
    features,
    get columns() { return tableColumns.value },
    // Do not materialize the library's unloaded rows just to manage its columns.
    data: [],
    manualSorting: true,
    enableMultiSort: false,
    enableSortingRemoval: false,
    columnResizeMode: 'onChange',
    get enableSorting() { return library.scope.kind !== 'similar' },
    initialState: { columnVisibility: defaultSampleColumns() },
    state: {
      get columnVisibility() { return visibility.value },
      get sorting() { return sorting.value },
    },
    onColumnVisibilityChange: (updater) => {
      const next = typeof updater === 'function' ? updater(visibility.value) : updater
      visibility.value = Object.fromEntries(SAMPLE_COLUMNS.map(column => [column.id, next[column.id] !== false])) as typeof visibility.value
    },
    onColumnSizingChange: (updater) => {
      const previous = table.atoms.columnSizing.get()
      const next = typeof updater === 'function' ? updater(previous) : updater
      let bounded: SampleColumnWidths = Object.fromEntries(columns.flatMap((column) => {
        const id = column.id as 'name' | typeof SAMPLE_COLUMNS[number]['id']
        if (next[id] == null) return []
        const maximum = next[id] === previous[id] ? sampleColumnLimits(id).max
          : sampleColumnResizeMax(id, layoutColumns.value, viewportWidth.value)
        return [[id, Math.min(maximum, boundedSampleColumnWidth(id, next[id]))]]
      }))
      if (resizingColumn) {
        const fitted = sampleTableLayout(layoutColumns.value, bounded, viewportWidth.value, resizingColumn).widths
        const last = layoutColumns.value.at(-1)?.id
        bounded = { ...bounded, ...Object.fromEntries(Object.entries(fitted).filter(([id]) => id !== last)) }
      }
      makeStateUpdater('columnSizing', table)(bounded)
      // Drag frames stay in the table atom; persist just the final width.
      if (!resizeStart) widths.value = bounded
    },
    onSortingChange: (updater) => {
      const next = typeof updater === 'function' ? updater(sorting.value) : updater
      if (!next[0]) return
      const sort = normalizeSampleSort({ by: next[0].id, direction: next[0].desc ? 'desc' : 'asc' })
      library.setSort(sort.by, sort.direction)
    },
  })

  watch(widths, (value) => {
    if (resizeStart) return
    const current = table.atoms.columnSizing.get()
    if (Object.keys(current).length !== Object.keys(value).length
      || Object.entries(value).some(([id, width]) => current[id] !== width))
      makeStateUpdater('columnSizing', table)({ ...value })
  }, { immediate: true, deep: true, flush: 'sync' })

  function beginResize(id: ResizableSampleColumnId, renderedWidths: SampleColumnWidths) {
    resizeStart = { ...widths.value }
    resizingColumn = id
    // Start from the displayed widths so flexible tracks never jump on press.
    const last = layoutColumns.value.at(-1)?.id
    const editable = Object.fromEntries(Object.entries(renderedWidths).filter(([column]) => column !== last))
    table.setColumnSizing(previous => ({ ...previous, ...editable }))
  }

  function finishResize(cancelled = false) {
    if (!resizeStart) return
    const next = cancelled ? resizeStart : table.atoms.columnSizing.get()
    resizeStart = null
    resizingColumn = undefined
    table.setColumnSizing(next)
  }

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

  function setViewportWidth(width: number) {
    viewportWidth.value = width
  }

  return { table, visibleColumns, beginResize, finishResize, resetWidths, resetView, setViewportWidth }
})
