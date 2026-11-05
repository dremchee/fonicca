export const SAMPLE_COLUMNS = [
  { id: 'directory', width: 180, defaultVisible: false },
  { id: 'tags', width: 120, defaultVisible: true },
  { id: 'category', width: 96, defaultVisible: true },
  { id: 'kind', width: 64, defaultVisible: false },
  { id: 'bpm', width: 56, defaultVisible: true },
  { id: 'key', width: 52, defaultVisible: false },
  { id: 'length', width: 64, defaultVisible: true },
  { id: 'type', width: 44, defaultVisible: false },
] as const

export type SampleColumnId = typeof SAMPLE_COLUMNS[number]['id']
export type SampleColumnVisibility = Record<SampleColumnId, boolean>

export function defaultSampleColumns(all = false): SampleColumnVisibility {
  return Object.fromEntries(SAMPLE_COLUMNS.map(c => [c.id, all || c.defaultVisible])) as SampleColumnVisibility
}

export type ResizableSampleColumnId = 'name' | SampleColumnId
export type SampleColumnWidths = Partial<Record<ResizableSampleColumnId, number>>

export function sampleColumnLimits(id: ResizableSampleColumnId) {
  return id === 'name' ? { min: 160, max: 2400 } : { min: id === 'directory' || id === 'tags' || id === 'category' ? 64 : 44, max: 800 }
}

export function boundedSampleColumnWidth(id: ResizableSampleColumnId, width: number): number {
  const limits = sampleColumnLimits(id)
  const fallback = id === 'name' ? 220 : SAMPLE_COLUMNS.find(c => c.id === id)!.width
  return Math.round(Math.min(limits.max, Math.max(limits.min, Number.isFinite(width) ? width : fallback)))
}

function sampleTableOverhead(columnCount: number) {
  // Playback, favorites, padding and the gaps between grid tracks.
  return 16 + 28 + 24 + (columnCount + 2) * 12
}

/** Reserve room for every other column, including the automatic final one. */
export function sampleColumnResizeMax(id: ResizableSampleColumnId, columns: readonly typeof SAMPLE_COLUMNS[number][], viewportWidth: number) {
  const limits = sampleColumnLimits(id)
  if (!Number.isFinite(viewportWidth) || viewportWidth <= 0) return limits.max
  const last = columns.at(-1)
  const reserved = sampleTableOverhead(columns.length)
    + (id === 'name' ? 0 : sampleColumnLimits('name').min)
    + columns.reduce((sum, column) => sum + (column.id === id ? 0
      : column.id === last?.id ? column.width : sampleColumnLimits(column.id).min), 0)
  return Math.max(limits.min, Math.min(limits.max, Math.floor(viewportWidth) - reserved))
}

export function sampleTableLayout(columns: readonly typeof SAMPLE_COLUMNS[number][], widths: SampleColumnWidths, viewportWidth?: number, resizing?: ResizableSampleColumnId) {
  const fixedName = columns.length > 0 && widths.name != null && Number.isFinite(widths.name)
  let nameWidth = fixedName ? boundedSampleColumnWidth('name', widths.name!) : 220
  const lastIndex = columns.length - 1
  const sizes = columns.map((c, i) => i === lastIndex ? c.width : boundedSampleColumnWidth(c.id, widths[c.id] ?? c.width))
  const overhead = sampleTableOverhead(columns.length)
  if (viewportWidth != null && Number.isFinite(viewportWidth) && viewportWidth > 0) {
    let overflow = Math.max(0, overhead + nameWidth + sizes.reduce((sum, size) => sum + size, 0) - Math.floor(viewportWidth))
    const fromName = resizing === 'name' || (fixedName && !resizing)
      ? 0 : Math.min(overflow, nameWidth - sampleColumnLimits('name').min)
    nameWidth -= fromName
    overflow -= fromName
    const capacity = sizes.map((size, i) => i === lastIndex || columns[i].id === resizing
      ? 0 : size - sampleColumnLimits(columns[i].id).min)
    let remaining = capacity.reduce((sum, size) => sum + size, 0)
    for (let i = 0; i < sizes.length && overflow > 0 && remaining > 0; i++) {
      const reduction = Math.min(capacity[i], Math.ceil(overflow * capacity[i] / remaining))
      sizes[i] -= reduction
      overflow -= reduction
      remaining -= capacity[i]
    }
    const remainingName = Math.min(overflow, nameWidth - sampleColumnLimits('name').min)
    nameWidth -= remainingName
    overflow -= remainingName
    const activeIndex = columns.findIndex(column => column.id === resizing)
    if (activeIndex >= 0 && activeIndex !== lastIndex) {
      const reduction = Math.min(overflow, sizes[activeIndex] - sampleColumnLimits(columns[activeIndex].id).min)
      sizes[activeIndex] -= reduction
      overflow -= reduction
    }
    // A narrow window or many enabled columns can exceed even the nominal
    // minimums. Clip earlier cells rather than pushing the final column out.
    const narrowNameMinimum = Math.min(80, Math.max(0, Math.floor(viewportWidth) - overhead - (sizes[lastIndex] ?? 0)))
    const fromNarrowName = Math.min(overflow, Math.max(0, nameWidth - narrowNameMinimum))
    nameWidth -= fromNarrowName
    overflow -= fromNarrowName
    remaining = sizes.reduce((sum, size, i) => sum + (i === lastIndex ? 0 : size), 0)
    for (let i = 0; i < lastIndex && overflow > 0 && remaining > 0; i++) {
      const size = sizes[i]
      const reduction = Math.min(size, Math.ceil(overflow * size / remaining))
      sizes[i] -= reduction
      overflow -= reduction
      remaining -= size
    }
  }
  return {
    widths: { name: nameWidth, ...Object.fromEntries(columns.map((column, i) => [column.id, sizes[i]])) } as SampleColumnWidths,
    '--sample-columns': `16px ${fixedName ? `${nameWidth}px` : `minmax(${nameWidth}px, 1fr)`} ${sizes.map((w, i) => fixedName && i === lastIndex ? `minmax(${w}px, 1fr)` : `${w}px`).join(' ')} 28px`,
    minWidth: `${overhead + nameWidth + sizes.reduce((sum, width) => sum + width, 0)}px`,
  }
}

export interface SampleSort {
  by: Exclude<ResizableSampleColumnId, 'directory'>
  direction: 'asc' | 'desc'
}

export function normalizeSampleSort(value: unknown): SampleSort {
  const sort = value && typeof value === 'object' ? value as Partial<SampleSort> : {}
  return {
    by: sort.by === 'name' || SAMPLE_COLUMNS.some(column => column.id !== 'directory' && column.id === sort.by) ? sort.by! : 'name',
    direction: sort.direction === 'desc' ? 'desc' : 'asc',
  }
}

export function sampleDirectory(path: string): string {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index < 0 ? '' : path.slice(0, index) || path.slice(0, 1)
}

export function relativeSampleDirectory(path: string, sourceRoot?: string): string {
  const directory = sampleDirectory(path)
  const root = sourceRoot?.replace(/[\\/]+$/, '')
  if (root && directory === root)
    return root.split(/[\\/]/).pop() ?? root
  if (root && (directory.startsWith(`${root}/`) || directory.startsWith(`${root}\\`)))
    return directory.slice(root.length + 1)
  return directory.split(/[\\/]/).filter(Boolean).pop() ?? directory
}
